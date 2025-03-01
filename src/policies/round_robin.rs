use std::sync::{
    atomic::{self, AtomicUsize},
    Arc,
};

use crate::{balancer::HostState, policy::BalancerPolicy};

pub struct RoundRobin {
    hosts: Vec<Arc<HostState>>,
    next: AtomicUsize,
}

impl RoundRobin {
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            next: AtomicUsize::new(0),
        }
    }

    pub fn set_hosts(&mut self, hosts: Vec<Arc<HostState>>) {
        self.hosts = hosts;
    }

    pub fn next(&self) -> Option<Arc<HostState>> {
        // it's ok if it overflows
        let next = self.next.fetch_add(1, atomic::Ordering::SeqCst) % self.hosts.len();
        self.hosts.get(next).cloned()
    }
}

impl BalancerPolicy for RoundRobin {
    fn next(&self) -> Option<Arc<HostState>> {
        RoundRobin::next(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::balancer::HostConfig;

    use super::*;

    #[test]
    fn test_happy() {
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

        let mut policy = RoundRobin::new();
        policy.set_hosts(hosts);
        let result = (0..4)
            .map(|_| policy.next().map(|h| h.config.host.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            result,
            vec![
                Some("a".to_owned()),
                Some("b".to_owned()),
                Some("c".to_owned()),
                Some("a".to_owned())
            ]
        );
    }

    #[test]
    fn test_one() {
        let hosts = vec![Arc::new(HostState::new(HostConfig {
            host: "a".to_owned(),
            weight: 1,
        }))];

        let mut policy = RoundRobin::new();
        policy.set_hosts(hosts);
        assert_eq!(
            policy.next().map(|h| h.config.host.clone()),
            Some("a".to_owned())
        );
        assert_eq!(
            policy.next().map(|h| h.config.host.clone()),
            Some("a".to_owned())
        );
    }
}
