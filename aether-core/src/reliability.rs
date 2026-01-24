//! Reliability utilities: retry, timeout, and circuit breaker.

use anyhow::{anyhow, Result};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl RetryPolicy {
    pub fn new(max_retries: usize, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
            max_delay,
        }
    }

    fn backoff_delay(&self, attempt: usize) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }
        let factor = 2_u32.saturating_pow((attempt - 1) as u32);
        let delay = self.base_delay.saturating_mul(factor);
        delay.min(self.max_delay)
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_threshold: usize,
    open_duration: Duration,
    half_open_successes: usize,
}

#[derive(Debug)]
enum CircuitState {
    Closed { failures: usize },
    Open { opened_at: Instant },
    HalfOpen { successes: usize },
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, open_duration: Duration, half_open_successes: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed { failures: 0 })),
            failure_threshold: failure_threshold.max(1),
            open_duration,
            half_open_successes: half_open_successes.max(1),
        }
    }

    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        {
            let mut state = self.state.lock().await;
            match &mut *state {
                CircuitState::Open { opened_at } => {
                    if opened_at.elapsed() < self.open_duration {
                        return Err(anyhow!("circuit open"));
                    }
                    *state = CircuitState::HalfOpen { successes: 0 };
                }
                _ => {}
            }
        }

        let result = f().await;

        let mut state = self.state.lock().await;
        match (&mut *state, result.is_ok()) {
            (CircuitState::Closed { failures }, true) => {
                *failures = 0;
            }
            (CircuitState::Closed { failures }, false) => {
                *failures += 1;
                if *failures >= self.failure_threshold {
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                }
            }
            (CircuitState::HalfOpen { successes }, true) => {
                *successes += 1;
                if *successes >= self.half_open_successes {
                    *state = CircuitState::Closed { failures: 0 };
                }
            }
            (CircuitState::HalfOpen { .. }, false) => {
                *state = CircuitState::Open {
                    opened_at: Instant::now(),
                };
            }
            (CircuitState::Open { .. }, _) => {}
        }

        result
    }
}

pub async fn retry_with_timeout<F, Fut, T, E>(
    policy: &RetryPolicy,
    timeout: Duration,
    mut f: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut attempt = 0;
    loop {
        let result = tokio::time::timeout(timeout, f()).await;
        match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(err)) => {
                if attempt >= policy.max_retries {
                    return Err(anyhow!(err));
                }
            }
            Err(_) => {
                if attempt >= policy.max_retries {
                    return Err(anyhow!("timeout"));
                }
            }
        }

        attempt += 1;
        let delay = policy.backoff_delay(attempt);
        if delay > Duration::from_millis(0) {
            sleep(delay).await;
        }
    }
}
