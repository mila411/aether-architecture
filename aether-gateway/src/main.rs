//! Aether Gateway - monitoring and management gateway for the Aether layer
//!
//! Observes all waves and provides statistics

use aether_core::{Aether, AetherConfig, Channel, Vibrator, VibratorConfig, Wave};
use std::collections::HashMap;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("🌊 Starting Aether Gateway...");

    // Initialize the Aether layer
    let aether = Aether::new(AetherConfig::default());

    // Vibrator that monitors all channels
    let config = VibratorConfig::new("aether-gateway").with_channels(vec![
        Channel::new("*"), // Monitor all channels
    ]);

    let mut vibrator = Vibrator::new(config, &aether).await;

    info!("✨ Gateway connected to the Aether layer");
    info!("👁️  Monitoring all channels...");

    // Statistics
    let mut stats = GatewayStats::new();

    // Stats report task
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            print_stats(&aether).await;
        }
    });

    // Main loop: observe all waves
    loop {
        if let Some(wave) = vibrator.receive().await {
            observe_wave(&mut stats, wave);
        }
    }
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

fn observe_wave(stats: &mut GatewayStats, wave: Wave) {
    info!(
        "👁️  [Observed] Channel: {} | Type: {:?} | Amplitude: {:.3} | Propagation: {} | Source: {:?}",
        wave.channel().name(),
        wave.wave_type(),
        wave.amplitude().value(),
        wave.propagation_count(),
        wave.source()
    );

    stats.record_wave(&wave);

    // Payload preview
    if let Some(obj) = wave.payload().as_object() {
        for (key, value) in obj.iter().take(3) {
            info!("   📦 {}: {}", key, value);
        }
    }
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
