use std::sync::Arc;

use serde::Deserialize;

use crate::{host::HostState, policy::BalancerPolicy, policy::RoundRobin};

use super::{DynamicWeightedRoundRobin, LeastConnections, WeightedRoundRobin};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyType {
    RoundRobin,
    WeightedRoundRobin,
    DynamicWeightedRoundRobin,
    LeastConnections,
}

pub struct PolicyFactory;

impl PolicyFactory {
    pub fn make(
        policy: PolicyType,
        hosts: Vec<Arc<HostState>>,
    ) -> Box<dyn BalancerPolicy + Send + Sync> {
        match policy {
            PolicyType::RoundRobin => {
                let mut policy = RoundRobin::new();
                policy.set_hosts(hosts);
                Box::new(policy)
            }
            PolicyType::WeightedRoundRobin => {
                let mut policy = WeightedRoundRobin::new();
                policy.set_hosts(hosts);
                Box::new(policy)
            }
            PolicyType::DynamicWeightedRoundRobin => {
                let mut policy = DynamicWeightedRoundRobin::new();
                policy.set_hosts(hosts);
                Box::new(policy)
            }
            PolicyType::LeastConnections => {
                let mut policy = LeastConnections::new();
                policy.set_hosts(hosts);
                Box::new(policy)
            }
        }
    }
}
