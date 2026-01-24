//! Task management with backpressure controls.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;
use tokio::time::Interval;
use tracing::warn;

#[derive(Debug)]
struct RateLimiter {
    interval: Mutex<Interval>,
}

impl RateLimiter {
    fn new(rate_per_sec: f64) -> Self {
        let interval = tokio::time::interval(Duration::from_secs_f64(1.0 / rate_per_sec));
        Self {
            interval: Mutex::new(interval),
        }
    }

    async fn acquire(&self) {
        let mut interval = self.interval.lock().await;
        interval.tick().await;
    }
}

#[derive(Debug)]
pub struct TaskManager {
    semaphore: Arc<Semaphore>,
    join_set: JoinSet<()>,
    rate_limiter: Option<RateLimiter>,
}

impl TaskManager {
    pub fn new(max_inflight: usize, rate_limit_per_sec: Option<f64>) -> Self {
        let max_inflight = max_inflight.max(1);
        let rate_limiter = rate_limit_per_sec.filter(|v| *v > 0.0).map(RateLimiter::new);

        Self {
            semaphore: Arc::new(Semaphore::new(max_inflight)),
            join_set: JoinSet::new(),
            rate_limiter,
        }
    }

    pub async fn spawn<F>(&mut self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Some(rate_limiter) = &self.rate_limiter {
            rate_limiter.acquire().await;
        }

        let permit = match self.semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };

        self.join_set.spawn(async move {
            let _permit = permit;
            fut.await;
        });
    }

    pub async fn reap(&mut self) {
        loop {
            match self.join_set.try_join_next() {
                Some(Err(err)) => {
                    warn!("Task failed: {}", err);
                }
                Some(Ok(_)) => continue,
                None => break,
            }
        }
    }
}
