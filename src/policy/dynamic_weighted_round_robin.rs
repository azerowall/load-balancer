use crate::{host::HostState, policy::BalancerPolicy};
use std::{
    sync::{
        atomic::{self, AtomicBool, AtomicIsize, AtomicU64, AtomicUsize},
        Arc,
    },
    time::{Duration, SystemTime},
};

use tracing::debug;

struct InnerHostState {
    host: Arc<HostState>,
    weight: AtomicUsize,
    current_weight: AtomicIsize,
}

pub struct DynamicWeightedRoundRobin {
    hosts: Vec<InnerHostState>,
    weights_update_interval: Duration,
    last_weights_update_time: AtomicU64,
    update_flag: AtomicBool,
}

impl DynamicWeightedRoundRobin {
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            weights_update_interval: Duration::from_secs(5),
            last_weights_update_time: AtomicU64::new(0),
            update_flag: AtomicBool::new(false),
        }
    }

    pub fn set_hosts(&mut self, hosts: Vec<Arc<HostState>>) {
        self.hosts = hosts
            .into_iter()
            .map(|host| InnerHostState {
                host,
                weight: AtomicUsize::new(1),
                current_weight: AtomicIsize::new(0),
            })
            .collect();
    }

    pub fn update_weights(&self) {
        for host in &self.hosts {
            let latency_ms = host.host.latency_ms.get_current();
            let weight = calculate_weight(host.host.config.weight, latency_ms);
            host.weight.store(weight, atomic::Ordering::SeqCst);
        }
    }

    fn get_last_weights_update_time(&self) -> SystemTime {
        let time = self.last_weights_update_time.load(atomic::Ordering::SeqCst);
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(time);
        time
    }

    fn set_last_weights_update_time(&self, time: SystemTime) {
        let duration = time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        self.last_weights_update_time
            .store(duration.as_secs(), atomic::Ordering::SeqCst);
    }

    fn update_weights_if_needed(&self) {
        let now = std::time::SystemTime::now();
        let last_update_time = self.get_last_weights_update_time();
        let elapsed = now.duration_since(last_update_time).unwrap();

        // If it's not enough time passed - don't update
        if elapsed < self.weights_update_interval {
            return;
        }

        // Here we try to aqcuire update_flag to prevent
        // weights update from several threads.
        // (it's no necessary, just optimization)
        let lock = self.update_flag.compare_exchange(
            false,
            true,
            atomic::Ordering::SeqCst,
            atomic::Ordering::SeqCst,
        );

        if let Err(_) = lock {
            return;
        }

        debug!("Periodic weights update");
        self.update_weights();
        self.set_last_weights_update_time(now);

        self.update_flag.store(false, atomic::Ordering::SeqCst);
    }

    pub fn next(&self) -> Option<Arc<HostState>> {
        self.update_weights_if_needed();

        let mut total_weight = 0;
        let mut selected: Option<&InnerHostState> = None;
        let mut max_current_weight: isize = -1;

        for host in &self.hosts {
            let weight = host.weight.load(atomic::Ordering::SeqCst);

            let current_weight = host
                .current_weight
                .fetch_add(weight as isize, atomic::Ordering::SeqCst)
                + weight as isize;

            total_weight += weight;

            if selected.is_none() || max_current_weight < current_weight {
                selected = Some(host);
                max_current_weight = current_weight;
            }
        }

        let selected = selected?;

        selected
            .current_weight
            .fetch_sub(total_weight as isize, atomic::Ordering::SeqCst);

        Some(selected.host.clone())
    }
}

fn calculate_weight(config_weight: usize, latency_ms: usize) -> usize {
    const WEIGHT_SCALE: usize = 10_000;
    // avoid division by 0
    let latency_ms = latency_ms.max(1);
    config_weight * WEIGHT_SCALE / latency_ms
}

impl BalancerPolicy for DynamicWeightedRoundRobin {
    fn next(&self) -> Option<Arc<HostState>> {
        DynamicWeightedRoundRobin::next(self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use crate::host::HostConfig;
    use crate::utils::statistics::Avg;

    use super::*;

    #[test]
    fn test_happy_path() {
        let hosts = vec![
            Arc::new(HostState {
                config: HostConfig {
                    host: "a".to_owned(),
                    weight: 1,
                },
                alive: AtomicBool::new(true),
                latency_ms: Avg::new(1),
                connections: AtomicUsize::new(0),
            }),
            Arc::new(HostState {
                config: HostConfig {
                    host: "b".to_owned(),
                    weight: 1,
                },
                alive: AtomicBool::new(true),
                latency_ms: Avg::new(1),
                connections: AtomicUsize::new(0),
            }),
        ];

        let mut policy = DynamicWeightedRoundRobin::new();
        policy.set_hosts(hosts.clone());

        assert_eq!(policy.next().unwrap().address(), "a");
        assert_eq!(policy.next().unwrap().address(), "b");
        assert_eq!(policy.next().unwrap().address(), "a");
        assert_eq!(policy.next().unwrap().address(), "b");

        hosts[0].latency_ms.account(3);
        policy.update_weights();

        assert_eq!(policy.next().unwrap().address(), "b");
        assert_eq!(policy.next().unwrap().address(), "a");
        assert_eq!(policy.next().unwrap().address(), "b");
        assert_eq!(policy.next().unwrap().address(), "b");
    }

    #[test]
    fn test_weighted_round_robin_similarity() {
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

        let mut policy = DynamicWeightedRoundRobin::new();
        policy.set_hosts(hosts);
        let result = (0..12)
            .map(|_| policy.next().unwrap().address().to_owned())
            .collect::<Vec<_>>();
        let expected =
            ["a", "c", "a", "b", "c", "a", "a", "c", "a", "b", "c", "a"].map(ToOwned::to_owned);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_one() {
        let hosts = vec![Arc::new(HostState::new(HostConfig {
            host: "a".to_owned(),
            weight: 2,
        }))];

        let mut policy = DynamicWeightedRoundRobin::new();
        policy.set_hosts(hosts);
        assert_eq!(
            policy.next().map(|h| h.address().to_owned()),
            Some("a".to_owned())
        );
        assert_eq!(
            policy.next().map(|h| h.address().to_owned()),
            Some("a".to_owned())
        );
    }
}
