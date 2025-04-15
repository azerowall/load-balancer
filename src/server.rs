use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use http::header::Entry;
use http::HeaderName;
use http::HeaderValue;
use http::Request;
use http::Response;
use http::StatusCode;
use http_body_util::Either;
use http_body_util::Full;
use hyper::body::Body;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::rt::TokioTimer;
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::app_state::AppState;
use crate::host::HostTracker;
use crate::metrics;
use crate::utils;

// Methods for passing X-Forwarded-* / Forwarded headers
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardedHeaderMethod {
    // Pass header as is.
    // Should be used if there's a trusted proxy before LB,
    // but we don't want to append its address to the header.
    Pass,
    // Remove header if it's presented.
    // Should be used if there's no trusted proxy before LB
    // and upstream doesn't need this header.
    // This is the default because it's the most secure way.
    #[default]
    Remove,
    // Remove existing header if it's presented and write
    // new one.
    // Should be used if there's no trusted proxy before LB
    // and upstream needs this header.
    Overwrite,
    // If header doesn't exist - create new one, if header
    // exists - append address of client to it.
    // Should be used if there's a trusted proxy before LB
    // and upstream needs the header with all proxies in it.
    // In case of X-Forwarded-{Host,Port,Proto} it has the same
    // effect as Overwirte.
    Append,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub listen_addr: String,
    #[serde(with = "utils::serde_millis")]
    pub upstream_timeout: Duration,
    pub upstream_max_idle_per_host: usize,
    #[serde(with = "utils::serde_millis")]
    pub server_header_read_timeout: Duration,
    #[serde(default)]
    pub set_header_x_forwarded_for: ForwardedHeaderMethod,
    #[serde(default)]
    pub set_header_x_forwarded_port: ForwardedHeaderMethod,
    #[serde(default)]
    pub set_header_x_forwarded_proto: ForwardedHeaderMethod,
    #[serde(default)]
    pub set_header_x_forwarded_host: ForwardedHeaderMethod,
    #[serde(default)]
    pub set_header_forwarded: ForwardedHeaderMethod,
    #[serde(default)]
    pub client_ip_header: Option<String>,
}

const X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
const X_FORWARDED_PORT: HeaderName = HeaderName::from_static("x-forwarded-port");
const X_FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");
const X_FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");

pub struct Server(Arc<ServerInner>);

struct ServerInner {
    config: Config,
    client: Client<HttpConnector, hyper::body::Incoming>,
}

impl Server {
    pub fn new(config: Config) -> Self {
        let client = Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(config.upstream_max_idle_per_host)
            .set_host(false)
            .build_http();

        Server(Arc::new(ServerInner { config, client }))
    }

    pub async fn run(&self, state: Arc<AppState>) -> crate::Result<()> {
        let listener = TcpListener::bind(&self.0.config.listen_addr).await?;
        info!("server is listening on {}", self.0.config.listen_addr);

        loop {
            let (stream, client_addr) = listener.accept().await?;

            debug!("got client {client_addr}");

            let server = self.clone();
            let state = state.clone();
            tokio::spawn(async move {
                let service = service_fn({
                    |req| {
                        let proxy = server.clone();
                        let state = state.clone();
                        proxy.serve_request(state, client_addr, req)
                    }
                });

                let io = TokioIo::new(stream);

                // TODO: By some reason prometheus causes HeaderTimeout error
                // even if it works fine and scrapes metrics.
                let mut http = http1::Builder::new();
                http.timer(TokioTimer::new())
                    .header_read_timeout(Some(server.0.config.server_header_read_timeout));
                let result = http.serve_connection(io, &service).await;
                if let Err(err) = result {
                    error!("error during handling client {client_addr}: {err:?}");
                } else {
                    debug!("connection closed {client_addr}");
                }
            });
        }
    }

    async fn serve_request(
        self,
        state: Arc<AppState>,
        client_addr: SocketAddr,
        req: Request<hyper::body::Incoming>,
    ) -> crate::Result<Response<ServerBody>> {
        match req.uri().path() {
            // NOTE: it's better to have /metrics endpoint and service API
            // on different port, but for now I'm ok with it.
            "/metrics" => self.handle_metrics().await,
            _ => self.proxy_response(state, client_addr, req).await,
        }
    }

    async fn handle_metrics(self) -> crate::Result<Response<ServerBody>> {
        let data = metrics::gather()?;
        let resp = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain")
            .body(ServerBody::Right(Full::new(Bytes::from(data))))?;
        Ok(resp)
    }

    async fn proxy_response(
        self,
        state: Arc<AppState>,
        client_addr: SocketAddr,
        mut req: Request<hyper::body::Incoming>,
    ) -> crate::Result<Response<ServerBody>> {
        let host = state.balancer.next_host();
        let host = match host {
            Some(host) => host,
            None => {
                warn!("no available upstream");
                return Self::bad_gateway();
            }
        };
        let host = HostTracker::start_request(host);

        let address = host.address();

        info!(
            "got request for {} - it will be proxied to {}",
            req.uri(),
            address
        );

        self.prepare_request(&mut req, client_addr, address)?;

        let request_fut = self.0.client.request(req);
        let result = tokio::time::timeout(self.0.config.upstream_timeout, request_fut).await;
        let response = match result {
            Ok(response) => response,
            Err(_) => {
                warn!("upstream {address} timed out");
                const TIMEOUT_REASON: &str = "timeout";
                metrics::UPSTREAM_ERRORS_COUNT
                    .with_label_values(&[address, TIMEOUT_REASON])
                    .inc();
                return Self::gateway_timeout();
            }
        };

        let response = match response {
            Ok(response) => response,
            Err(e) => {
                warn!("upstream {address} error: {e}");
                let reason = client_error_reason(&e);
                metrics::UPSTREAM_ERRORS_COUNT
                    .with_label_values(&[address, reason])
                    .inc();
                return Self::bad_gateway();
            }
        };
        debug!("upstream {} response {}", address, response.status(),);

        host.response_headers_received();

        let response = response.map(|body| ServerBody::Left(TrackedIncoming::new(host, body)));
        Ok(response)
    }

    fn prepare_request(
        &self,
        req: &mut Request<hyper::body::Incoming>,
        client_addr: SocketAddr,
        upstream_address: &str,
    ) -> crate::Result<()> {
        // Add schema and host to uri for hyper client.
        let mut uri_parts = req.uri().clone().into_parts();
        uri_parts.scheme = Some(http::uri::Scheme::HTTP);
        uri_parts.authority = Some(http::uri::Authority::from_str(upstream_address)?);
        *req.uri_mut() = http::uri::Uri::from_parts(uri_parts)?;

        let client_ip = client_addr.ip().to_string();

        // Set X-Forwarded-For header
        match self.0.config.set_header_x_forwarded_for {
            ForwardedHeaderMethod::Pass => {}
            ForwardedHeaderMethod::Remove => {
                req.headers_mut().remove(X_FORWARDED_FOR);
            }
            ForwardedHeaderMethod::Overwrite => {
                req.headers_mut().insert(
                    X_FORWARDED_FOR,
                    HeaderValue::from_str(&client_ip).expect("client ip is valid header value"),
                );
            }
            ForwardedHeaderMethod::Append => {
                //
                match req.headers_mut().entry(X_FORWARDED_FOR) {
                    Entry::Vacant(vacant) => {
                        let value = HeaderValue::from_str(&client_ip)
                            .expect("client ip is valid header value");
                        vacant.insert(value);
                    }
                    Entry::Occupied(mut occupied) => {
                        let mut value = occupied.get().as_bytes().to_owned();
                        value.extend_from_slice(b",");
                        value.extend_from_slice(client_ip.as_bytes());

                        *occupied.get_mut() = HeaderValue::from_bytes(&value)?;
                    }
                }
            }
        }

        // Set X-Forwarded-Port header
        match self.0.config.set_header_x_forwarded_port {
            ForwardedHeaderMethod::Pass => {}
            ForwardedHeaderMethod::Remove => {
                req.headers_mut().remove(X_FORWARDED_PORT);
            }
            ForwardedHeaderMethod::Overwrite | ForwardedHeaderMethod::Append => {
                req.headers_mut().insert(
                    X_FORWARDED_PORT,
                    HeaderValue::from_str(&client_addr.port().to_string())?,
                );
            }
        }

        // Set X-Forwarded-Proto header
        match self.0.config.set_header_x_forwarded_proto {
            ForwardedHeaderMethod::Pass => {}
            ForwardedHeaderMethod::Remove => {
                req.headers_mut().remove(X_FORWARDED_PROTO);
            }
            ForwardedHeaderMethod::Overwrite | ForwardedHeaderMethod::Append => {
                // We can't get scheme from uri because uri is sent in form of "/path/to/smth" usually.
                // The only way - to have knowlege about socket if it's using SSL/TLS.
                // By now we don't support it, so just set "http".
                req.headers_mut().insert(
                    X_FORWARDED_PROTO,
                    HeaderValue::from_str("http").expect("http is valid header value"),
                );
            }
        }

        // Set X-Forwarded-Host header
        match self.0.config.set_header_x_forwarded_host {
            ForwardedHeaderMethod::Pass => {}
            ForwardedHeaderMethod::Remove => {
                req.headers_mut().remove(X_FORWARDED_HOST);
            }
            ForwardedHeaderMethod::Overwrite | ForwardedHeaderMethod::Append => {
                if let Some(host_header) = req.headers().get(http::header::HOST) {
                    let host_header = host_header.clone();
                    req.headers_mut().insert(X_FORWARDED_HOST, host_header);
                }
            }
        }

        // Set Forwarded header
        match self.0.config.set_header_forwarded {
            ForwardedHeaderMethod::Pass => {}
            ForwardedHeaderMethod::Remove => {
                req.headers_mut().remove(http::header::FORWARDED);
            }
            ForwardedHeaderMethod::Overwrite => {
                let mut value = Vec::new();
                write_forwarded_header_item(
                    &mut value,
                    &client_ip,
                    req.headers().get(http::header::HOST),
                );
                req.headers_mut()
                    .insert(http::header::FORWARDED, HeaderValue::try_from(value)?);
            }
            ForwardedHeaderMethod::Append => {
                let host_header = req.headers().get(http::header::HOST).cloned();

                match req.headers_mut().entry(http::header::FORWARDED) {
                    Entry::Vacant(vacant) => {
                        let mut value = Vec::new();
                        write_forwarded_header_item(&mut value, &client_ip, host_header.as_ref());
                        vacant.insert(HeaderValue::try_from(value)?);
                    }
                    Entry::Occupied(mut occupied) => {
                        let mut value = occupied.get().as_bytes().to_owned();
                        value.extend_from_slice(b",");
                        write_forwarded_header_item(&mut value, &client_ip, host_header.as_ref());
                        occupied
                            .insert(HeaderValue::from_bytes(&value).expect("valid header value"));
                    }
                }
            }
        }

        // Set client ip header
        if let Some(header_name) = &self.0.config.client_ip_header {
            req.headers_mut().insert(
                HeaderName::from_str(header_name)?,
                HeaderValue::from_str(&client_addr.ip().to_string())
                    .expect("client ip is valid header value"),
            );
        }

        Ok(())
    }

    fn bad_gateway() -> crate::Result<Response<ServerBody>> {
        let resp = Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(ServerBody::Right(Full::default()))?;

        Ok(resp)
    }

    fn gateway_timeout() -> crate::Result<Response<ServerBody>> {
        let resp = Response::builder()
            .status(StatusCode::GATEWAY_TIMEOUT)
            .body(ServerBody::Right(Full::default()))?;

        Ok(resp)
    }
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

fn write_forwarded_header_item(
    value: &mut Vec<u8>,
    client_ip: &str,
    host_header: Option<&HeaderValue>,
) {
    value.extend_from_slice(b"for=");
    value.extend_from_slice(client_ip.as_bytes());
    if let Some(host_header) = host_header {
        value.extend_from_slice(b";host=");
        value.extend_from_slice(host_header.as_bytes());
    }
    value.extend_from_slice(b";proto=http");
}

fn client_error_reason(err: &hyper_util::client::legacy::Error) -> &'static str {
    if err.is_connect() {
        return "connect";
    }
    return "other";
}

// TrackedIncoming

// This is a wrapper over hyper::body::Incoming,
// which tracks the end of body stream.
// It does it by storing HostTracker. So, when `Drop::drop()` is called
// HostTracker guard is going to be dropped too.
// NOTE: We could use `Body::poll_frame()` and check when Incoming
// returns an `Poll::Read(None)`, but it's not guaranteed
// that stream will be polled fully by hyper - instead hyper can
// rely on Body::is_end_stream and refuse doing polling.
struct TrackedIncoming {
    _host: HostTracker,
    body: Incoming,
}

impl TrackedIncoming {
    pub fn new(host: HostTracker, body: Incoming) -> Self {
        Self { _host: host, body }
    }
}

impl Body for TrackedIncoming {
    type Data = <Incoming as Body>::Data;

    type Error = <Incoming as Body>::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let body = Pin::new(&mut this.body);
        body.poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> hyper::body::SizeHint {
        self.body.size_hint()
    }
}

// We use our own body type, because we want to return either
// `body::Incoming` or `http_body_util::Empty`.
// We might have done it using BoxBody, but it will lead
// to allocation per request.
type ServerBody = Either<TrackedIncoming, Full<Bytes>>;
