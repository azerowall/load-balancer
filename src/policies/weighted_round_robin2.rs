use std::sync::{Arc, Mutex};

use crate::{balancer::HostState, policy::BalancerPolicy, utils};

struct State {
    current_index: usize,
    current_weight: usize,
}

pub struct WeightedRoundRobin2 {
    hosts: Vec<Arc<HostState>>,
    state: Mutex<State>,
    max_weight: usize,
    gcd_weight: usize,
}

impl WeightedRoundRobin2 {
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            state: Mutex::new(State {
                current_index: 0,
                current_weight: 0,
            }),
            max_weight: 0,
            gcd_weight: 0,
        }
    }

    pub fn set_hosts(&mut self, hosts: Vec<Arc<HostState>>) {
        self.hosts = hosts;

        let state = self.state.get_mut().unwrap();
        state.current_index = 0;
        state.current_weight = 0;

        let weights: Vec<_> = self.hosts.iter().map(|h| h.config.weight).collect();

        let max_weight = *weights.iter().max().unwrap();
        let gcd_weight = utils::math::gcd_nums(weights.iter().copied()).unwrap();

        self.max_weight = max_weight;
        self.gcd_weight = gcd_weight;
    }

    pub fn next(&self) -> Option<Arc<HostState>> {
        let mut state = self.state.lock().unwrap();

        if self.hosts.is_empty() {
            return None;
        }

        loop {
            let index = state.current_index;
            state.current_index = (state.current_index + 1) % self.hosts.len();
            if state.current_index == 0 {
                if state.current_weight > self.max_weight {
                    state.current_weight = 0;
                } else {
                    state.current_weight += self.gcd_weight;
                }
            }

            let weight = self.hosts[index].config.weight;
            if weight >= state.current_weight {
                return self.hosts.get(index).cloned();
            }
        }
    }
}

impl BalancerPolicy for WeightedRoundRobin2 {
    fn next(&self) -> Option<Arc<HostState>> {
        WeightedRoundRobin2::next(self)
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

        let mut policy = WeightedRoundRobin2::new();
        policy.set_hosts(hosts);
        let result = (0..24)
            .map(|_| policy.next().map(|h| h.config.host.clone()))
            .collect::<Vec<_>>();
        let expected = [
            "a", "b", "c", "a", "b", "c", "a", "a", "c", "a", "b", "c", "a", "b", "c", "a", "a",
            "c", "a", "b", "c", "a", "b", "c",
        ]
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

        let mut policy = WeightedRoundRobin2::new();
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
