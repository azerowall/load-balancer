use std::sync::{
    atomic::{self, AtomicBool, AtomicIsize, AtomicUsize},
    Arc,
};

use arc_swap::ArcSwap;
use serde::Deserialize;

use crate::{
    policies::factory::{PolicyFactory, PolicyType},
    policy::BalancerPolicy,
};

#[derive(Clone, Deserialize)]
pub struct HostConfig {
    pub host: String,
    pub weight: usize,
}

pub struct HostState {
    pub config: HostConfig,
    pub alive: AtomicBool,
    pub weight: AtomicUsize,
    pub current_weight: AtomicIsize,
    pub connections: AtomicUsize,
}

impl HostState {
    pub fn new(config: HostConfig) -> Self {
        let weight = config.weight;
        Self {
            config,
            alive: AtomicBool::new(true),
            weight: AtomicUsize::new(weight),
            current_weight: AtomicIsize::new(0),
            connections: AtomicUsize::new(0),
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub policy: PolicyType,
}

pub struct Balancer {
    config: Config,
    hosts: Vec<Arc<HostState>>,
    // NOTE: unfortunately we can't have ArcSwap<dyn ...> because
    // T in ArcSwap<T> has to be Sized.
    // Thus we have Arc<Box<...>> here.
    policy: ArcSwap<Box<dyn BalancerPolicy + Send + Sync>>,
}

impl Balancer {
    pub fn new(config: Config, hosts: Vec<HostConfig>) -> Self {
        let hosts: Vec<Arc<HostState>> = hosts
            .into_iter()
            .map(HostState::new)
            .map(Arc::new)
            .collect();

        let policy = PolicyFactory::make(config.policy, hosts.clone());
        let policy = ArcSwap::new(Arc::new(policy));

        Self {
            config,
            hosts,
            policy,
        }
    }

    pub fn next_host(&self) -> Option<Arc<HostState>> {
        let policy = self.policy.load();
        policy.next()
    }

    pub fn hosts(&self) -> Vec<Arc<HostState>> {
        self.hosts.clone()
    }

    pub fn update_alive_hosts(&self) {
        let alive_hosts = self.alive_hosts();

        let policy = PolicyFactory::make(self.config.policy, alive_hosts);
        let policy = Arc::new(policy);

        self.policy.swap(policy);
    }

    fn alive_hosts(&self) -> Vec<Arc<HostState>> {
        self.hosts
            .iter()
            .filter(|&h| h.alive.load(atomic::Ordering::SeqCst))
            .cloned()
            .collect()
    }
}
