use std::sync::Arc;

use crate::host::HostState;

pub trait BalancerPolicy {
    fn next(&self) -> Option<Arc<HostState>>;
}
