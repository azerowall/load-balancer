use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Avg {
    sum: AtomicUsize,
    count: AtomicUsize,
}

impl Avg {
    pub fn new(latency: usize) -> Self {
        Self {
            sum: AtomicUsize::new(latency),
            count: AtomicUsize::new(1),
        }
    }

    pub fn account(&self, value: usize) {
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Release);
    }

    pub fn get_current(&self) -> usize {
        let sum = self.sum.load(Ordering::Relaxed);
        let count = self.count.load(Ordering::Acquire);

        match count {
            0 => 0,
            count => sum / count,
        }
    }
}

impl Default for Avg {
    fn default() -> Self {
        Self {
            sum: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }
}
