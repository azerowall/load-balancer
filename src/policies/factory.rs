use std::sync::Arc;

use serde::Deserialize;

use crate::{host::HostState, policies::RoundRobin, policy::BalancerPolicy};

use super::{
    dynamic_weighted_round_robin::DynamicWeightedRoundRobin, LeastConnections, WeightedRoundRobin,
    WeightedRoundRobin2,
};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyType {
    RoundRobin,
    WeightedRoundRobin,
    WeightedRoundRobin2,
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
            PolicyType::WeightedRoundRobin2 => {
                let mut policy = WeightedRoundRobin2::new();
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
