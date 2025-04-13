pub mod dynamic_weighted_round_robin;
pub mod factory;
pub mod least_connections;
pub mod round_robin;
pub mod weighted_round_robin;

use std::sync::Arc;

use crate::host::HostState;

pub use dynamic_weighted_round_robin::DynamicWeightedRoundRobin;
pub use least_connections::LeastConnections;
pub use round_robin::RoundRobin;
pub use weighted_round_robin::WeightedRoundRobin;

pub trait BalancerPolicy {
    fn next(&self) -> Option<Arc<HostState>>;
}
