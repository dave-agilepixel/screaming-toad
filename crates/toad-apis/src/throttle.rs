//! Request pacing: at most `concurrency` requests in flight, and request
//! starts at least `interval` apart (a requests-per-minute limit).

use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};

pub struct Throttle {
    slots: Semaphore,
    concurrency: usize,
    interval: Duration,
    next: Mutex<Instant>,
}

impl Throttle {
    pub fn new(concurrency: usize, per_minute: u32) -> Self {
        let concurrency = concurrency.max(1);
        let interval = if per_minute == 0 { Duration::ZERO } else { Duration::from_secs_f64(60.0 / per_minute as f64) };
        Self { slots: Semaphore::new(concurrency), concurrency, interval, next: Mutex::new(Instant::now()) }
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency
    }

    pub async fn acquire(&self) -> SemaphorePermit<'_> {
        let permit = self.slots.acquire().await.expect("throttle closed");
        if !self.interval.is_zero() {
            let wait = {
                let mut next = self.next.lock().await;
                let now = Instant::now();
                let at = (*next).max(now);
                *next = at + self.interval;
                at - now
            };
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        }
        permit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spaces_requests() {
        let t = Throttle::new(4, 600); // one per 100 ms
        let start = Instant::now();
        for _ in 0..4 {
            let _p = t.acquire().await;
        }
        assert!(start.elapsed() >= Duration::from_millis(290), "{:?}", start.elapsed());
    }
}
