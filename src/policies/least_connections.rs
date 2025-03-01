use std::sync::{atomic, Arc};

use crate::{balancer::HostState, policy::BalancerPolicy};

pub struct LeastConnections {
    hosts: Vec<Arc<HostState>>,
}

impl LeastConnections {
    pub fn new() -> Self {
        Self { hosts: Vec::new() }
    }

    pub fn set_hosts(&mut self, hosts: Vec<Arc<HostState>>) {
        self.hosts = hosts;
    }

    pub fn next(&self) -> Option<Arc<HostState>> {
        let min = self
            .hosts
            .iter()
            .min_by_key(|&h| h.connections.load(atomic::Ordering::SeqCst));
        min.cloned()
    }
}

impl BalancerPolicy for LeastConnections {
    fn next(&self) -> Option<Arc<HostState>> {
        LeastConnections::next(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::balancer::HostConfig;

    use super::*;

    #[test]
    fn test_simple() {
        let hosts = vec![
            Arc::new(HostState::new(HostConfig {
                host: "a".to_owned(),
                weight: 1,
            })),
            Arc::new(HostState::new(HostConfig {
                host: "b".to_owned(),
                weight: 1,
            })),
            Arc::new(HostState::new(HostConfig {
                host: "c".to_owned(),
                weight: 1,
            })),
        ];

        hosts[0].connections.store(2, atomic::Ordering::SeqCst);
        hosts[1].connections.store(0, atomic::Ordering::SeqCst);
        hosts[2].connections.store(1, atomic::Ordering::SeqCst);

        let mut policy = LeastConnections::new();
        policy.set_hosts(hosts.clone());

        assert_eq!(
            policy.next().map(|h| h.config.host.clone()),
            Some("b".to_owned())
        );

        hosts[1].connections.fetch_add(2, atomic::Ordering::SeqCst);
        assert_eq!(
            policy.next().map(|h| h.config.host.clone()),
            Some("c".to_owned())
        );
    }
}
