use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use serde::Deserialize;
use tracing::debug;

use crate::{metrics, utils};

#[derive(Clone, Deserialize)]
pub struct HostConfig {
    pub host: String,
    pub weight: usize,
}

pub struct HostState {
    pub config: HostConfig,
    pub alive: AtomicBool,

    // Host metrics:
    // for dynamic_weighted_round_robin
    // TODO: calculate moving average
    pub latency_ms: utils::statistics::Avg,
    // for least_connections
    pub connections: AtomicUsize,
}

impl HostState {
    pub fn new(config: HostConfig) -> Self {
        Self {
            config,
            alive: AtomicBool::new(true),
            latency_ms: utils::statistics::Avg::default(),
            connections: AtomicUsize::new(0),
        }
    }

    pub fn address(&self) -> &str {
        &self.config.host
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn set_alive(&self, alive: bool) {
        self.alive.store(alive, Ordering::SeqCst);
    }
}

// Tracks metrics for host
// including guarding of connections counter
pub struct HostTracker {
    host: Arc<HostState>,
    request_start: Instant,
}

impl HostTracker {
    pub fn start_request(host: Arc<HostState>) -> Self {
        host.connections.fetch_add(1, Ordering::SeqCst);

        Self {
            host,
            request_start: Instant::now(),
        }
    }

    pub fn response_headers_received(&self) {
        let elapsed = self.request_start.elapsed();

        // NOTE: now we measure latency as time between request and headers receiving.
        // Later we can add ability to configure it.
        self.host.latency_ms.account(elapsed.as_millis() as usize);

        let address = self.host.address();
        metrics::UPSTREAM_TIMINGS_SECONDS
            .with_label_values(&[address])
            .observe(elapsed.as_secs_f64());
        metrics::UPSTREAM_RPS_COUNT
            .with_label_values(&[address])
            .inc();
    }

    fn response_body_received(&self) {
        debug!("host {} request finished", self.host.address());
        self.host.connections.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Deref for HostTracker {
    type Target = Arc<HostState>;
    fn deref(&self) -> &Self::Target {
        &self.host
    }
}

impl Drop for HostTracker {
    fn drop(&mut self) {
        self.response_body_received();
    }
}
