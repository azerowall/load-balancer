use std::{
    convert::Infallible,
    io,
    sync::{atomic, Arc},
    time::Duration,
};

use http::uri;
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::body::Bytes;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::app_state::AppState;
use crate::utils;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub path: String,
    #[serde(with = "utils::serde_millis")]
    pub interval: Duration,
}

pub struct Healthcheck {
    app_state: Arc<AppState>,
    config: Config,
    client: Client<HttpConnector, BoxBody<Bytes, Infallible>>,
}

impl Healthcheck {
    pub fn new(config: Config, app_state: Arc<AppState>) -> Self {
        let client = Client::builder(TokioExecutor::new()).build_http();

        Self {
            app_state,
            config,
            client,
        }
    }

    pub async fn run(&self) {
        loop {
            tokio::time::sleep(self.config.interval).await;
            self.do_healthcheck().await;
        }
    }

    async fn do_healthcheck(&self) {
        // go throught all hosts and make request to healthcheck path

        let hosts = self.app_state.balancer.hosts();

        for host in &hosts {
            let result = self.check(&host.config.host).await;
            if let Err(e) = result {
                warn!("healthcheck failed for {}: {}", host.config.host, e);
                host.alive.store(false, atomic::Ordering::SeqCst);
            } else {
                debug!("healthcheck successful for {}", host.config.host);
                host.alive.store(true, atomic::Ordering::SeqCst);
            }
        }

        self.app_state.balancer.update_alive_hosts();
    }

    async fn check(&self, host: &str) -> crate::Result<()> {
        let uri = uri::Uri::builder()
            .scheme(uri::Scheme::HTTP)
            .authority(host)
            .path_and_query(self.config.path.clone())
            .build()?;

        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri(uri)
            .body(Empty::new().boxed())?;

        let resp = self.client.request(req).await?;

        if !resp.status().is_success() {
            return Err(io::Error::other(format!("got bad response {}", resp.status())).into());
        }

        Ok(())
    }
}
