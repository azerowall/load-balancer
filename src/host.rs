use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde::Deserialize;

use crate::utils;

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
