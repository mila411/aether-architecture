//! Service Alpha - order processing service
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
use tracing::{error, info, warn};

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    let app_config = load_config("service-alpha").context("failed to load service config")?;

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

    info!("🌊 Starting Service Alpha (order processing service)...");

    // Watch config changes
    let mut config_rx =
        watch_config("service-alpha").context("failed to start config watcher")?;
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
        vec![Channel::new("orders.*"), Channel::new("payments.completed")]
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

    let service_name = config.name.clone();
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

    info!("✨ Service Alpha connected to the Aether layer");
    info!("📡 Resonant channels: {:?}", vibrator.resonant_channels());

    // Send a demo order creation wave
    tokio::spawn({
        let aether = aether.clone();
        let service_name = service_name.clone();
        let retry_policy = retry_policy.clone();
        let breaker = breaker.clone();
        let timeout = timeout;
        let auth_token = app_config.aether.auth_token.clone();
        async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            info!("📦 Creating a new order...");
            let order = json!({
                "order_id": "ORD-12345",
                "customer": "John Doe",
                "items": ["ItemA", "ItemB"],
                "total": 15000,
                "status": "pending"
            });

            let mut wave = Wave::builder(Channel::new("orders.created"))
                .payload(order)
                .source(service_name)
                .build();

            if let Some(token) = auth_token {
                wave.set_auth_token(token);
            }

            let send_result = breaker
                .call(|| async {
                    retry_with_timeout(&retry_policy, timeout, || aether.emit(wave.clone())).await
                })
                .await;

            if let Err(e) = send_result {
                if is_recoverable(&e) {
                    warn!("Failed to send order creation (recoverable): {}", e);
                } else {
                    error!("Failed to send order creation (unrecoverable): {}", e);
                }
            }
        }
    });

    // Shutdown handling
    let (shutdown_tx, shutdown_rx) = shutdown_signal();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    // Main loop: receive and process waves
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
                    task_manager
                        .spawn(async move {
                            handle_wave(&emitter, wave, &retry_policy, timeout, &breaker).await;
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
        ch if ch.starts_with("orders.") => {
            handle_order_wave(vibrator, wave, retry_policy, timeout, breaker).await
        }
        "payments.completed" => {
            handle_payment_completed(vibrator, wave, retry_policy, timeout, breaker).await
        }
        _ => {
            info!("Unknown channel: {}", channel);
        }
    }
}

async fn handle_order_wave(
    vibrator: &VibratorEmitter,
    wave: Wave,
    retry_policy: &RetryPolicy,
    timeout: std::time::Duration,
    breaker: &CircuitBreaker,
) {
    let payload = wave.payload();

    match wave.channel().name() {
        "orders.created" => {
            info!("📦 Processing new order: {:?}", payload);

            // Validate order
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Send inventory check wave
            let inventory_check = json!({
                "order_id": payload.get("order_id"),
                "items": payload.get("items"),
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            let send_result = breaker
                .call(|| async {
                    retry_with_timeout(retry_policy, timeout, || {
                        vibrator.emit_wave(Channel::new("inventory.check"), inventory_check.clone())
                    })
                    .await
                })
                .await;

            if let Err(e) = send_result {
                if is_recoverable(&e) {
                    warn!("Failed to send inventory check (recoverable): {}", e);
                } else {
                    error!("Failed to send inventory check (unrecoverable): {}", e);
                }
            } else {
                info!("📊 Inventory check request sent");
            }
        }
        "orders.confirmed" => {
            info!("✅ Order confirmed: {:?}", payload);

            // Send payment request
            let payment_request = json!({
                "order_id": payload.get("order_id"),
                "amount": payload.get("total"),
                "method": "credit_card"
            });

            let send_result = breaker
                .call(|| async {
                    retry_with_timeout(retry_policy, timeout, || {
                        vibrator.emit_wave(Channel::new("payments.request"), payment_request.clone())
                    })
                    .await
                })
                .await;

            if let Err(e) = send_result {
                if is_recoverable(&e) {
                    warn!("Failed to send payment request (recoverable): {}", e);
                } else {
                    error!("Failed to send payment request (unrecoverable): {}", e);
                }
            } else {
                info!("💳 Payment request sent");
            }
        }
        _ => {}
    }
}

async fn handle_payment_completed(
    vibrator: &VibratorEmitter,
    wave: Wave,
    retry_policy: &RetryPolicy,
    timeout: std::time::Duration,
    breaker: &CircuitBreaker,
) {
    info!("💰 Received payment completion: {:?}", wave.payload());

    // Send order completion wave
    let order_completed = json!({
        "order_id": wave.payload().get("order_id"),
        "status": "completed",
        "completed_at": chrono::Utc::now().to_rfc3339()
    });

    let send_result = breaker
        .call(|| async {
            retry_with_timeout(retry_policy, timeout, || {
                vibrator.emit_wave(Channel::new("orders.completed"), order_completed.clone())
            })
            .await
        })
        .await;

    if let Err(e) = send_result {
        if is_recoverable(&e) {
            warn!("Failed to send order completion (recoverable): {}", e);
        } else {
            error!("Failed to send order completion (unrecoverable): {}", e);
        }
    } else {
        info!("🎉 Order completed!");
    }
}

fn is_recoverable(err: &anyhow::Error) -> bool {
    err.downcast_ref::<aether_core::AetherError>()
        .map(|e| e.is_recoverable())
        .unwrap_or(false)
}
