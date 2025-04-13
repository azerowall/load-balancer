use serde::Deserialize;

use crate::{balancer, healthcheck, host, server};

#[derive(Deserialize)]
pub struct AppConfig {
    pub server: server::Config,
    pub balancer: balancer::Config,
    pub healthcheck: healthcheck::Config,
    pub hosts: Vec<host::HostConfig>,
}
