use std::sync::{
    atomic::{self},
    Arc,
};

use crate::{balancer::HostState, policy::BalancerPolicy};

pub struct WeightedRoundRobin {
    hosts: Vec<Arc<HostState>>,
    total_weight: usize,
}

impl WeightedRoundRobin {
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            total_weight: 0,
        }
    }

    pub fn set_hosts(&mut self, hosts: Vec<Arc<HostState>>) {
        self.hosts = hosts;

        self.total_weight = self.hosts.iter().fold(0, |a, b| a + b.config.weight);
    }

    pub fn next(&self) -> Option<Arc<HostState>> {
        let mut next: Option<&Arc<HostState>> = None;

        let mut max_weight: isize = -1;
        for host in &self.hosts {
            let weight = host.config.weight;
            let mut current_weight = host
                .current_weight
                .fetch_add(weight as isize, atomic::Ordering::SeqCst);
            current_weight += weight as isize;

            if current_weight > max_weight {
                max_weight = current_weight;
                next = Some(host);
            }
        }
        let next = next?;

        next.current_weight
            .fetch_sub(self.total_weight as isize, atomic::Ordering::SeqCst);

        Some(next.clone())
    }
}

impl BalancerPolicy for WeightedRoundRobin {
    fn next(&self) -> Option<Arc<HostState>> {
        WeightedRoundRobin::next(self)
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
                weight: 3,
            })),
            Arc::new(HostState::new(HostConfig {
                host: "b".to_owned(),
                weight: 1,
            })),
            Arc::new(HostState::new(HostConfig {
                host: "c".to_owned(),
                weight: 2,
            })),
        ];

        let mut policy = WeightedRoundRobin::new();
        policy.set_hosts(hosts);
        let result = (0..12)
            .map(|_| policy.next().map(|h| h.config.host.clone()))
            .collect::<Vec<_>>();
        let expected = ["a", "c", "a", "b", "c", "a", "a", "c", "a", "b", "c", "a"]
            .map(ToOwned::to_owned)
            .map(Some);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_one() {
        let hosts = vec![Arc::new(HostState::new(HostConfig {
            host: "a".to_owned(),
            weight: 2,
        }))];

        let mut policy = WeightedRoundRobin::new();
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
