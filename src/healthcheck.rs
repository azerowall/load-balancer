use std::{convert::Infallible, io, sync::Arc, time::Duration};

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
    pub enabled: bool,
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
        if !self.config.enabled {
            return;
        }

        loop {
            tokio::time::sleep(self.config.interval).await;
            self.do_healthcheck().await;
        }
    }

    async fn do_healthcheck(&self) {
        // go throught all hosts and make request to healthcheck path

        let hosts = self.app_state.balancer.hosts();

        for host in &hosts {
            let result = self.check(host.address()).await;
            if let Err(e) = result {
                warn!("healthcheck failed for {}: {}", host.address(), e);
                host.set_alive(false);
            } else {
                debug!("healthcheck successful for {}", host.address());
                host.set_alive(true);
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
