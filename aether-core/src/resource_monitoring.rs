//! Resource monitoring: memory usage, leak detection, allocator metrics.

use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct ResourceMonitorConfig {
    pub enabled: bool,
    pub interval_ms: u64,
    pub leak_detection_enabled: bool,
    pub leak_growth_bytes_per_min: u64,
    pub allocator_metrics_enabled: bool,
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
            leak_detection_enabled: false,
            leak_growth_bytes_per_min: 10 * 1024 * 1024,
            allocator_metrics_enabled: false,
        }
    }
}

pub fn start_resource_monitoring(config: ResourceMonitorConfig) -> Option<JoinHandle<()>> {
    if !config.enabled {
        return None;
    }

    Some(tokio::spawn(async move {
        let pid = sysinfo::get_current_pid().ok();
        let mut system = System::new();
        let mut last_mem: Option<(u64, Instant)> = None;

        loop {
            if let Some(pid) = pid {
                system.refresh_process(pid);
                if let Some(process) = system.process(pid) {
                    let rss_kb = process.memory();
                    let vmem_kb = process.virtual_memory();
                    let rss_bytes = rss_kb.saturating_mul(1024);
                    let vmem_bytes = vmem_kb.saturating_mul(1024);

                    metrics::gauge!("process_memory_rss_bytes").set(rss_bytes as f64);
                    metrics::gauge!("process_memory_vms_bytes").set(vmem_bytes as f64);

                    if config.leak_detection_enabled {
                        let now = Instant::now();
                        if let Some((prev_mem, prev_time)) = last_mem {
                            let elapsed = now.duration_since(prev_time).as_secs_f64();
                            if elapsed > 1.0 {
                                let growth_per_min = ((rss_bytes.saturating_sub(prev_mem)) as f64)
                                    / elapsed
                                    * 60.0;
                                metrics::gauge!("process_memory_growth_bytes_per_min")
                                    .set(growth_per_min);
                                if growth_per_min as u64 > config.leak_growth_bytes_per_min {
                                    metrics::counter!("process_memory_leak_suspected_total")
                                        .increment(1);
                                    warn!(
                                        "Possible memory leak: growth {:.0} bytes/min",
                                        growth_per_min
                                    );
                                }
                            }
                        }
                        last_mem = Some((rss_bytes, now));
                    }

                    if config.allocator_metrics_enabled {
                        #[cfg(feature = "jemalloc")]
                        {
                            if let Err(err) = record_jemalloc_metrics() {
                                warn!("Failed to collect jemalloc metrics: {}", err);
                            }
                        }
                        #[cfg(not(feature = "jemalloc"))]
                        {
                            warn!("Allocator metrics enabled but jemalloc feature is disabled");
                        }
                    }
                }
            }

            sleep(Duration::from_millis(config.interval_ms)).await;
        }
    }))
}

#[cfg(feature = "jemalloc")]
fn record_jemalloc_metrics() -> Result<(), String> {
    use jemalloc_ctl::{epoch, stats};

    epoch::advance().map_err(|e| e.to_string())?;
    let allocated = stats::allocated::read().map_err(|e| e.to_string())?;
    let active = stats::active::read().map_err(|e| e.to_string())?;
    let resident = stats::resident::read().map_err(|e| e.to_string())?;

    metrics::gauge!("allocator_allocated_bytes").set(allocated as f64);
    metrics::gauge!("allocator_active_bytes").set(active as f64);
    metrics::gauge!("allocator_resident_bytes").set(resident as f64);

    Ok(())
}
