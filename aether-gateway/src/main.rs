//! Aether Gateway - monitoring and management gateway for the Aether layer
//!
//! Observes all waves and provides statistics

use aether_core::{
    apply_resource_limits, init_observability, init_ops, install_panic_hook, load_config,
    shutdown_signal, start_resource_monitoring, wait_for_shutdown, watch_config, Aether, Channel,
    OpsConfig, ResourceMonitorConfig, TaskManager, Vibrator, VibratorConfig, Wave,
};
use anyhow::Context;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    let app_config = load_config("aether-gateway").context("failed to load service config")?;

    // Panic hook & resource limits
    install_panic_hook();
    apply_resource_limits(
        app_config.operations.memory_limit_bytes,
        app_config.operations.cpu_time_limit_secs,
    )
    .context("failed to apply resource limits")?;

    // Initialize observability (logging/metrics/tracing)
    let _observability = init_observability(&app_config).context("failed to init observability")?;

    // Operations (health check)
    let _ops = init_ops(&OpsConfig {
        enable_health: app_config.operations.health_enabled,
        health_bind: app_config.operations.health_bind.clone(),
        shutdown_grace_ms: app_config.operations.shutdown_grace_ms,
        memory_limit_bytes: app_config.operations.memory_limit_bytes,
        cpu_time_limit_secs: app_config.operations.cpu_time_limit_secs,
    });

    let _resource_monitor = start_resource_monitoring(ResourceMonitorConfig {
        enabled: app_config.resource_monitoring.enabled,
        interval_ms: app_config.resource_monitoring.interval_ms,
        leak_detection_enabled: app_config.resource_monitoring.leak_detection_enabled,
        leak_growth_bytes_per_min: app_config.resource_monitoring.leak_growth_bytes_per_min,
        allocator_metrics_enabled: app_config.resource_monitoring.allocator_metrics_enabled,
    });

    info!("🌊 Starting Aether Gateway...");

    // Watch config changes
    let mut config_rx = watch_config("aether-gateway").context("failed to start config watcher")?;
    tokio::spawn(async move {
        while config_rx.changed().await.is_ok() {
            let updated = config_rx.borrow().clone();
            info!("🔄 Config reloaded for {}", updated.service.name);
        }
    });

    // Initialize the Aether layer
    let aether = Aether::new(app_config.aether_config());

    // Vibrator that monitors all channels
    let channels = if app_config.service.channels.is_empty() {
        vec![Channel::new("*")]
    } else {
        app_config
            .service
            .channels
            .iter()
            .map(|ch| Channel::new(ch))
            .collect()
    };
    let config = VibratorConfig::new(app_config.service.name.clone())
        .with_channels(channels)
        .with_auth_token(app_config.aether.auth_token.clone())
        .with_noise_floor(app_config.service.noise_floor);

    let mut vibrator = Vibrator::new(config, &aether).await;
    let mut task_manager = TaskManager::new(
        app_config.service.max_inflight,
        app_config.service.rate_limit_per_sec,
    );

    info!("✨ Gateway connected to the Aether layer");
    info!("👁️  Monitoring all channels...");

    // Statistics
    let stats = Arc::new(Mutex::new(GatewayStats::new()));

    // Stats report task
    let aether_clone = aether.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            print_stats(&aether_clone).await;
        }
    });

    // Shutdown handling
    let (shutdown_tx, shutdown_rx) = shutdown_signal();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    // Main loop: observe all waves
    loop {
        tokio::select! {
            _ = wait_for_shutdown(shutdown_rx.clone()) => {
                info!("Shutdown signal received");
                break;
            }
            wave = vibrator.receive() => {
                if let Some(wave) = wave {
                    let stats = Arc::clone(&stats);
                    task_manager
                        .spawn(async move {
                            observe_wave(stats, wave).await;
                        })
                        .await;
                    task_manager.reap().await;
                }
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(
        app_config.operations.shutdown_grace_ms,
    ))
    .await;

    Ok(())
}

#[derive(Debug)]
struct GatewayStats {
    total_waves: u64,
    waves_by_channel: HashMap<String, u64>,
    waves_by_type: HashMap<String, u64>,
    average_amplitude: f64,
}

impl GatewayStats {
    fn new() -> Self {
        Self {
            total_waves: 0,
            waves_by_channel: HashMap::new(),
            waves_by_type: HashMap::new(),
            average_amplitude: 0.0,
        }
    }

    fn record_wave(&mut self, wave: &Wave) {
        self.total_waves += 1;

        // Count by channel
        *self
            .waves_by_channel
            .entry(wave.channel().name().to_string())
            .or_insert(0) += 1;

        // Count by type
        *self
            .waves_by_type
            .entry(format!("{:?}", wave.wave_type()))
            .or_insert(0) += 1;

        // Update average amplitude
        let n = self.total_waves as f64;
        self.average_amplitude =
            (self.average_amplitude * (n - 1.0) + wave.amplitude().value()) / n;
    }
}

async fn observe_wave(stats: Arc<Mutex<GatewayStats>>, wave: Wave) {
    info!(
        "👁️  [Observed] Channel: {} | Type: {:?} | Amplitude: {:.3} | Propagation: {} | Source: {:?}",
        wave.channel().name(),
        wave.wave_type(),
        wave.amplitude().value(),
        wave.propagation_count(),
        wave.source()
    );

    let mut stats_guard = stats.lock().await;
    stats_guard.record_wave(&wave);
}

async fn print_stats(aether: &Aether) {
    let stats = aether.stats().await;
    let channels = aether.active_channels().await;

    info!("📊 ===== Aether Layer Stats =====");
    info!("   Total waves: {}", stats.total_waves);
    info!("   Active channels: {}", stats.active_channels);
    info!("   Channel list: {:?}", channels);
    info!("=============================");
}
