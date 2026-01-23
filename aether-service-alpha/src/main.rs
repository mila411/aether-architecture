//! Service Alpha - order processing service
//!
//! Example microservice implementation using Aether architecture

use aether_core::{Aether, AetherConfig, Channel, Vibrator, VibratorConfig, Wave};
use serde_json::json;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("🌊 Starting Service Alpha (order processing service)...");

    // Initialize the Aether layer
    let aether = Aether::new(AetherConfig::default());

    // Create vibrator
    let config = VibratorConfig::new("service-alpha").with_channels(vec![
        Channel::new("orders.*"),
        Channel::new("payments.completed"),
    ]);

    let service_name = config.name.clone();
    let mut vibrator = Vibrator::new(config, &aether).await;

    info!("✨ Service Alpha connected to the Aether layer");
    info!("📡 Resonant channels: {:?}", vibrator.resonant_channels());

    // Send a demo order creation wave
    tokio::spawn({
        let aether = aether.clone();
        let service_name = service_name.clone();
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

            let wave = Wave::builder(Channel::new("orders.created"))
                .payload(order)
                .source(service_name)
                .build();

            if let Err(e) = aether.emit(wave).await {
                error!("Failed to send order creation: {}", e);
            }
        }
    });

    // Main loop: receive and process waves
    loop {
        if let Some(wave) = vibrator.receive().await {
            handle_wave(&vibrator, wave).await;
        }
    }
}

async fn handle_wave(vibrator: &Vibrator, wave: Wave) {
    let channel = wave.channel().name();

    info!(
        "🌊 Received wave: channel={}, type={:?}, amplitude={:.2}",
        channel,
        wave.wave_type(),
        wave.amplitude().value()
    );

    match channel {
        ch if ch.starts_with("orders.") => handle_order_wave(vibrator, wave).await,
        "payments.completed" => handle_payment_completed(vibrator, wave).await,
        _ => {
            info!("Unknown channel: {}", channel);
        }
    }
}

async fn handle_order_wave(vibrator: &Vibrator, wave: Wave) {
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

            if let Err(e) = vibrator
                .emit_wave(Channel::new("inventory.check"), inventory_check)
                .await
            {
                error!("Failed to send inventory check: {}", e);
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

            if let Err(e) = vibrator
                .emit_wave(Channel::new("payments.request"), payment_request)
                .await
            {
                error!("Failed to send payment request: {}", e);
            } else {
                info!("💳 Payment request sent");
            }
        }
        _ => {}
    }
}

async fn handle_payment_completed(vibrator: &Vibrator, wave: Wave) {
    info!("💰 Received payment completion: {:?}", wave.payload());

    // Send order completion wave
    let order_completed = json!({
        "order_id": wave.payload().get("order_id"),
        "status": "completed",
        "completed_at": chrono::Utc::now().to_rfc3339()
    });

    if let Err(e) = vibrator
        .emit_wave(Channel::new("orders.completed"), order_completed)
        .await
    {
        error!("Failed to send order completion: {}", e);
    } else {
        info!("🎉 Order completed!");
    }
}
