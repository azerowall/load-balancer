use std::sync::Arc;

use crate::balancer::HostState;

pub trait BalancerPolicy {
    fn next(&self) -> Option<Arc<HostState>>;
}
