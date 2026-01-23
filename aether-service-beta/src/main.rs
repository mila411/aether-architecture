//! Service Beta - inventory management service
//!
//! Example microservice implementation using Aether architecture

use aether_core::{Aether, AetherConfig, Channel, Vibrator, VibratorConfig, Wave};
use serde_json::json;
use std::collections::HashMap;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("🌊 Starting Service Beta (inventory management service)...");

    // Initialize the Aether layer
    let aether = Aether::new(AetherConfig::default());

    // Create vibrator
    let config = VibratorConfig::new("service-beta").with_channels(vec![
        Channel::new("inventory.*"),
        Channel::new("orders.created"),
    ]);

    let mut vibrator = Vibrator::new(config, &aether).await;

    info!("✨ Service Beta connected to the Aether layer");
    info!("📡 Resonant channels: {:?}", vibrator.resonant_channels());

    // Inventory data (simplified)
    let mut inventory = HashMap::new();
    inventory.insert("ItemA", 100);
    inventory.insert("ItemB", 50);
    inventory.insert("ItemC", 200);

    // Main loop: receive and process waves
    loop {
        if let Some(wave) = vibrator.receive().await {
            handle_wave(&vibrator, &mut inventory, wave).await;
        }
    }
}

async fn handle_wave(vibrator: &Vibrator, inventory: &mut HashMap<&str, i32>, wave: Wave) {
    let channel = wave.channel().name();

    info!(
        "🌊 Received wave: channel={}, type={:?}, amplitude={:.2}",
        channel,
        wave.wave_type(),
        wave.amplitude().value()
    );

    match channel {
        "inventory.check" => handle_inventory_check(vibrator, inventory, wave).await,
        "inventory.reserve" => handle_inventory_reserve(vibrator, inventory, wave).await,
        ch if ch.starts_with("orders.") => handle_order_event(vibrator, wave).await,
        _ => {
            info!("Unknown channel: {}", channel);
        }
    }
}

async fn handle_inventory_check(vibrator: &Vibrator, inventory: &HashMap<&str, i32>, wave: Wave) {
    info!("📊 Processing inventory check request...");

    let payload = wave.payload();
    let items = payload.get("items").and_then(|v| v.as_array());

    if let Some(items) = items {
        let mut all_available = true;
        let mut stock_info = Vec::new();

        for item in items {
            if let Some(item_name) = item.as_str() {
                let stock = inventory.get(item_name).copied().unwrap_or(0);
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

        if let Err(e) = vibrator.emit_wave(channel, result).await {
            error!("Failed to send inventory check result: {}", e);
        } else {
            info!("✅ Inventory check result sent");
        }

        // If inventory is available, also send order confirmation
        if all_available {
            if let Err(e) = vibrator
                .emit_wave(
                    Channel::new("orders.confirmed"),
                    json!({
                        "order_id": payload.get("order_id"),
                        "total": payload.get("total"),
                    }),
                )
                .await
            {
                error!("Failed to send order confirmation: {}", e);
            }
        }
    }
}

async fn handle_inventory_reserve(
    vibrator: &Vibrator,
    inventory: &mut HashMap<&str, i32>,
    wave: Wave,
) {
    info!("🔒 Processing inventory reservation request...");

    let payload = wave.payload();
    let items = payload.get("items").and_then(|v| v.as_array());

    if let Some(items) = items {
        for item in items {
            if let Some(item_name) = item.as_str() {
                if let Some(stock) = inventory.get_mut(item_name) {
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

        if let Err(e) = vibrator
            .emit_wave(Channel::new("inventory.reserved"), result)
            .await
        {
            error!("Failed to send reservation completion: {}", e);
        }
    }
}

async fn handle_order_event(_vibrator: &Vibrator, wave: Wave) {
    let channel = wave.channel().name();

    if channel == "orders.created" {
        info!("📦 New order detected");
        // Optionally auto-reserve inventory, etc.
    }
}
