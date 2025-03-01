use std::str::FromStr;
use std::sync::atomic;
use std::sync::Arc;
use std::time::Duration;

use http::Request;
use http::Response;
use http::StatusCode;
use http_body_util::Either;
use http_body_util::Empty;
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
use crate::utils;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub listen_addr: String,
    pub client_max_idle_per_host: usize,
    #[serde(with = "utils::serde_millis")]
    pub server_header_read_timeout: Duration,
    // TODO:
    // pub client_ip_header: Option<String>,
    // pub x_forwarded_for: bool,
}

pub struct Server(Arc<ServerInner>);

struct ServerInner {
    config: Config,
    client: Client<HttpConnector, hyper::body::Incoming>,
}

impl Server {
    pub fn new(config: Config) -> Self {
        let client = Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(config.client_max_idle_per_host)
            .build_http();

        Server(Arc::new(ServerInner { config, client }))
    }

    pub async fn run(&self, state: Arc<AppState>) -> crate::Result<()> {
        let listener = TcpListener::bind(&self.0.config.listen_addr).await?;
        info!("server is listening on {}", self.0.config.listen_addr);

        loop {
            let (stream, addr) = listener.accept().await?;

            info!("got client {addr}");

            let server = self.clone();
            let state = state.clone();
            tokio::spawn(async move {
                let service = service_fn({
                    |req| {
                        let proxy = server.clone();
                        let state = state.clone();
                        proxy.proxy_response(state, req)
                    }
                });

                let io = TokioIo::new(stream);

                let mut http = http1::Builder::new();
                http.timer(TokioTimer::new())
                    .header_read_timeout(Some(server.0.config.server_header_read_timeout));
                let result = http.serve_connection(io, &service).await;
                if let Err(err) = result {
                    error!("error during handling client {addr}: {err:?}");
                }
            });
        }
    }

    async fn proxy_response(
        self,
        state: Arc<AppState>,
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

        host.connections.fetch_add(1, atomic::Ordering::SeqCst);
        let address = &host.config.host;

        info!(
            "got request for {} - it will be proxied to {}",
            req.uri(),
            address
        );

        // add schema and host to uri - it's required by client
        let mut uri_parts = req.uri().clone().into_parts();
        uri_parts.scheme = Some(http::uri::Scheme::HTTP);
        uri_parts.authority = Some(http::uri::Authority::from_str(address)?);
        *req.uri_mut() = http::uri::Uri::from_parts(uri_parts)?;

        let response = self.0.client.request(req).await;
        let response = match response {
            Err(e) => {
                warn!("upstream {address} error: {e}");
                return Self::bad_gateway();
            }
            Ok(response) => response.map(ServerBody::Left),
        };

        debug!("upstream {} response {}", address, response.status());

        // NOTE: we return `body::Incoming`, which means at this point
        // we haven't finished this response.
        // Thus this metric doesn't show the real state of the things at the moment.
        // Instead we should update metrics at the end of the stream.
        host.connections.fetch_sub(1, atomic::Ordering::SeqCst);

        Ok(response)
    }

    fn bad_gateway() -> crate::Result<Response<ServerBody>> {
        let resp = Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(ServerBody::Right(Empty::new()))?;

        Ok(resp)
    }
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// We use our own body type, because we want to return either
// `body::Incoming` or `http_body_util::Empty`.
// We might have done it using BoxBody, but it will lead
// to allocation per request.
type ServerBody = Either<Incoming, Empty<Bytes>>;
