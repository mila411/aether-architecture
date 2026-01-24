//! Operations: graceful shutdown, health checks, panic hook, and resource limits.

use anyhow::{anyhow, Result};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct OpsConfig {
    pub enable_health: bool,
    pub health_bind: String,
    pub shutdown_grace_ms: u64,
    pub memory_limit_bytes: Option<u64>,
    pub cpu_time_limit_secs: Option<u64>,
}

pub struct OpsHandle {
    _health_task: Option<JoinHandle<()>>,
}

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "unknown panic"
        };

        let location = info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown".to_string());

        eprintln!("panic at {}: {}", location, payload);
    }));
}

pub fn apply_resource_limits(memory_bytes: Option<u64>, cpu_seconds: Option<u64>) -> Result<()> {
    #[cfg(unix)]
    unsafe {
        if let Some(bytes) = memory_bytes {
            let limit = libc::rlimit {
                rlim_cur: bytes as libc::rlim_t,
                rlim_max: bytes as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                return Err(anyhow!("failed to set RLIMIT_AS"));
            }
        }

        if let Some(seconds) = cpu_seconds {
            let limit = libc::rlimit {
                rlim_cur: seconds as libc::rlim_t,
                rlim_max: seconds as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_CPU, &limit) != 0 {
                return Err(anyhow!("failed to set RLIMIT_CPU"));
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = memory_bytes;
        let _ = cpu_seconds;
    }

    Ok(())
}

pub fn shutdown_signal() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}

pub fn spawn_health_server(bind: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        match TcpListener::bind(&bind).await {
            Ok(listener) => {
                info!("Health server listening on {}", bind);
                loop {
                    match listener.accept().await {
                        Ok((mut socket, _)) => {
                            tokio::spawn(async move {
                                let response =
                                    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                                if let Err(err) = tokio::io::AsyncWriteExt::write_all(
                                    &mut socket,
                                    response.as_bytes(),
                                )
                                .await
                                {
                                    warn!("Health response error: {}", err);
                                }
                            });
                        }
                        Err(err) => {
                            warn!("Health accept error: {}", err);
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                    }
                }
            }
            Err(err) => {
                warn!("Failed to bind health server {}: {}", bind, err);
            }
        }
    })
}

pub fn init_ops(config: &OpsConfig) -> OpsHandle {
    let health_task = if config.enable_health {
        Some(spawn_health_server(config.health_bind.clone()))
    } else {
        None
    };

    OpsHandle {
        _health_task: health_task,
    }
}

pub async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    let _ = shutdown_rx.changed().await;
}

pub async fn trigger_shutdown(shutdown_tx: watch::Sender<bool>) {
    let _ = shutdown_tx.send(true);
}
