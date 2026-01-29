use aether_core::{Aether, AetherConfig, Channel, Vibrator, VibratorConfig, Wave};
use serde_json::json;
use tokio::time::{sleep, Duration, timeout};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let aether = Aether::new(AetherConfig {
        use_nats: false,
        ..AetherConfig::default()
    });

    let hop_count = 4;
    let hop_interval_ms = 200;

    let mut receiver = Vibrator::new(
        VibratorConfig::new("receiver").with_noise_floor(0.05),
        &aether,
    )
    .await;
    receiver
        .resonate_hopping(Channel::new("orders"), hop_count)
        .await;

    let sender = Vibrator::new(VibratorConfig::new("sender"), &aether).await;

    // Low amplitude wave (filtered by noise floor)
    let low_channel = Channel::new("orders").hop_now(hop_count, hop_interval_ms);
    let low_wave = Wave::builder(low_channel)
        .payload(json!({"msg": "low"}))
        .amplitude(0.01)
        .build();
    sender.emit(low_wave).await?;

    // Wait briefly, then send a visible wave
    sleep(Duration::from_millis(50)).await;

    sender
        .emit_time_hopping_wave(
            Channel::new("orders"),
            hop_count,
            hop_interval_ms,
            json!({"msg": "hop"}),
        )
        .await?;

    let received = timeout(Duration::from_millis(200), receiver.receive()).await;
    match received {
        Ok(Some(wave)) => {
            println!("Received on {}: {}", wave.channel().name(), wave.payload());
        }
        _ => {
            println!("No wave received in time");
        }
    }

    Ok(())
}
