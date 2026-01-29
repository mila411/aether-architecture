//! Service Beta - inventory management service
//!
//! Example microservice implementation using Aether architecture

use aether_core::{
    apply_resource_limits, init_observability, init_ops, install_panic_hook, load_config,
    shutdown_signal, start_resource_monitoring, wait_for_shutdown, watch_config, Aether, Channel,
    OpsConfig, ResourceMonitorConfig, TaskManager, Vibrator, VibratorConfig, VibratorEmitter, Wave,
    CircuitBreaker, RetryPolicy, retry_with_timeout,
};
use anyhow::Context;
use serde_json::json;
use std::collections::HashMap;
use tracing::{error, info, warn};

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    let app_config = load_config("service-beta").context("failed to load service config")?;

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

    info!("🌊 Starting Service Beta (inventory management service)...");

    // Watch config changes
    let mut config_rx =
        watch_config("service-beta").context("failed to start config watcher")?;
    tokio::spawn(async move {
        while config_rx.changed().await.is_ok() {
            let updated = config_rx.borrow().clone();
            info!("🔄 Config reloaded for {}", updated.service.name);
        }
    });

    // Initialize the Aether layer
    let aether = Aether::new(app_config.aether_config());

    // Create vibrator
    let channels = if app_config.service.channels.is_empty() {
        vec![Channel::new("inventory.*"), Channel::new("orders.created")]
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
    let emitter = vibrator.emitter();
    let mut task_manager = TaskManager::new(
        app_config.service.max_inflight,
        app_config.service.rate_limit_per_sec,
    );

    let retry_policy = RetryPolicy::new(
        app_config.service.retry_max,
        std::time::Duration::from_millis(app_config.service.retry_base_delay_ms),
        std::time::Duration::from_millis(app_config.service.retry_max_delay_ms),
    );
    let timeout = std::time::Duration::from_millis(app_config.service.timeout_ms);
    let breaker = CircuitBreaker::new(
        app_config.service.circuit_breaker_failure_threshold,
        std::time::Duration::from_millis(app_config.service.circuit_breaker_open_ms),
        app_config.service.circuit_breaker_half_open_successes,
    );

    info!("✨ Service Beta connected to the Aether layer");
    info!("📡 Resonant channels: {:?}", vibrator.resonant_channels());

    // Inventory data (simplified)
    let mut inventory = HashMap::new();
    inventory.insert("ItemA", 100);
    inventory.insert("ItemB", 50);
    inventory.insert("ItemC", 200);

    // Main loop: receive and process waves
    let inventory = std::sync::Arc::new(tokio::sync::Mutex::new(inventory));

    let (shutdown_tx, shutdown_rx) = shutdown_signal();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    loop {
        tokio::select! {
            _ = wait_for_shutdown(shutdown_rx.clone()) => {
                info!("Shutdown signal received");
                break;
            }
            wave = vibrator.receive() => {
                if let Some(wave) = wave {
                    let emitter = emitter.clone();
                    let retry_policy = retry_policy.clone();
                    let breaker = breaker.clone();
                    let timeout = timeout;
                    let inventory = std::sync::Arc::clone(&inventory);
                    task_manager
                        .spawn(async move {
                            handle_wave(&emitter, inventory, wave, &retry_policy, timeout, &breaker).await;
                        })
                        .await;
                    task_manager.reap().await;
                }
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(app_config.operations.shutdown_grace_ms)).await;

    Ok(())
}

async fn handle_wave(
    vibrator: &VibratorEmitter,
    inventory: std::sync::Arc<tokio::sync::Mutex<HashMap<&str, i32>>>,
    wave: Wave,
    retry_policy: &RetryPolicy,
    timeout: std::time::Duration,
    breaker: &CircuitBreaker,
) {
    let channel = wave.channel().name();

    info!(
        "🌊 Received wave: channel={}, type={:?}, amplitude={:.2}",
        channel,
        wave.wave_type(),
        wave.amplitude().value()
    );

    match channel {
        "inventory.check" => {
            handle_inventory_check(vibrator, inventory, wave, retry_policy, timeout, breaker).await
        }
        "inventory.reserve" => {
            handle_inventory_reserve(vibrator, inventory, wave, retry_policy, timeout, breaker).await
        }
        ch if ch.starts_with("orders.") => handle_order_event(vibrator, wave).await,
        _ => {
            info!("Unknown channel: {}", channel);
        }
    }
}

async fn handle_inventory_check(
    vibrator: &VibratorEmitter,
    inventory: std::sync::Arc<tokio::sync::Mutex<HashMap<&str, i32>>>,
    wave: Wave,
    retry_policy: &RetryPolicy,
    timeout: std::time::Duration,
    breaker: &CircuitBreaker,
) {
    info!("📊 Processing inventory check request...");

    let payload = wave.payload();
    let items = payload.get("items").and_then(|v| v.as_array());

    if let Some(items) = items {
        let mut all_available = true;
        let mut stock_info = Vec::new();

        let inventory_guard = inventory.lock().await;
        for item in items {
            if let Some(item_name) = item.as_str() {
                let stock = inventory_guard.get(item_name).copied().unwrap_or(0);
                stock_info.push(json!({
                    "item": item_name,
                    "stock": stock,
                    "available": stock > 0
                }));

                if stock == 0 {
                    all_available = false;
                    warn!("⚠️  {} is out of stock", item_name);
                }
            }
        }

        // Send inventory check result
        let result = json!({
            "order_id": payload.get("order_id"),
            "available": all_available,
            "items": stock_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        let channel = if all_available {
            Channel::new("inventory.available")
        } else {
            Channel::new("inventory.unavailable")
        };

        let send_result = breaker
            .call(|| async {
                retry_with_timeout(retry_policy, timeout, || {
                    vibrator.emit_wave(channel.clone(), result.clone())
                })
                .await
            })
            .await;

        if let Err(e) = send_result {
            if is_recoverable(&e) {
                warn!("Failed to send inventory check result (recoverable): {}", e);
            } else {
                error!("Failed to send inventory check result (unrecoverable): {}", e);
            }
        } else {
            info!("✅ Inventory check result sent");
        }

        // If inventory is available, also send order confirmation
        if all_available {
            let send_result = breaker
                .call(|| async {
                    retry_with_timeout(retry_policy, timeout, || {
                        vibrator.emit_wave(
                            Channel::new("orders.confirmed"),
                            json!({
                                "order_id": payload.get("order_id"),
                                "total": payload.get("total"),
                            }),
                        )
                    })
                    .await
                })
                .await;

            if let Err(e) = send_result {
                if is_recoverable(&e) {
                    warn!("Failed to send order confirmation (recoverable): {}", e);
                } else {
                    error!("Failed to send order confirmation (unrecoverable): {}", e);
                }
            }
        }
    }
}

async fn handle_inventory_reserve(
    vibrator: &VibratorEmitter,
    inventory: std::sync::Arc<tokio::sync::Mutex<HashMap<&str, i32>>>,
    wave: Wave,
    retry_policy: &RetryPolicy,
    timeout: std::time::Duration,
    breaker: &CircuitBreaker,
) {
    info!("🔒 Processing inventory reservation request...");

    let payload = wave.payload();
    let items = payload.get("items").and_then(|v| v.as_array());

    if let Some(items) = items {
        let mut inventory_guard = inventory.lock().await;
        for item in items {
            if let Some(item_name) = item.as_str() {
                if let Some(stock) = inventory_guard.get_mut(item_name) {
                    if *stock > 0 {
                        *stock -= 1;
                        info!("📦 Reserved one {} (remaining: {})", item_name, stock);
                    }
                }
            }
        }

        // Reservation completion notice
        let result = json!({
            "order_id": payload.get("order_id"),
            "reserved": true,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        let send_result = breaker
            .call(|| async {
                retry_with_timeout(retry_policy, timeout, || {
                    vibrator.emit_wave(Channel::new("inventory.reserved"), result.clone())
                })
                .await
            })
            .await;

        if let Err(e) = send_result {
            if is_recoverable(&e) {
                warn!("Failed to send reservation completion (recoverable): {}", e);
            } else {
                error!("Failed to send reservation completion (unrecoverable): {}", e);
            }
        }
    }
}

fn is_recoverable(err: &anyhow::Error) -> bool {
    err.downcast_ref::<aether_core::AetherError>()
        .map(|e| e.is_recoverable())
        .unwrap_or(false)
}

async fn handle_order_event(_vibrator: &VibratorEmitter, wave: Wave) {
    let channel = wave.channel().name();

    if channel == "orders.created" {
        info!("📦 New order detected");
        // Optionally auto-reserve inventory, etc.
    }
}
