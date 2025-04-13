mod app_state;
mod balancer;
mod config;
mod healthcheck;
mod host;
mod metrics;
mod policies;
mod policy;
mod result;
mod server;
mod utils;

use std::{fs, sync::Arc};

use app_state::AppState;
use balancer::Balancer;
use config::AppConfig;
use healthcheck::Healthcheck;
use result::Result;
use server::Server;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let config = read_config()?;

    let balancer = Balancer::new(config.balancer.clone(), config.hosts.clone());

    let state = Arc::new(AppState { balancer });

    let healthcheck = Healthcheck::new(config.healthcheck.clone(), state.clone());

    tokio::spawn(async move { healthcheck.run().await });

    let server = Server::new(config.server.clone());
    server.run(state.clone()).await?;

    Ok(())
}

fn init_logging() {
    // If you want to see only application logs you can set env variable like this:
    // RUST_LOG=load_balancer=debug
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}

fn read_config() -> Result<AppConfig> {
    let mut config_file = None;

    // I don't see any reason for using clap for one argument only
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        match &arg[..] {
            "-c" => {
                let value = args.next().ok_or("config file expected")?;
                config_file = Some(value);
            }
            _ => {}
        }
    }

    let config_file = config_file.ok_or("config file wasn't provided")?;
    let config = fs::read_to_string(&config_file)?;

    Ok(toml::from_str(&config)?)
}
