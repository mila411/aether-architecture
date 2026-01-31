use aether_core::{Aether, AetherConfig, Channel, Vibrator};
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let aether = Aether::new(AetherConfig {
        use_nats: true,
        nats_url: "tls://localhost:4223".to_string(),
        nats_tls_required: true,
        nats_mtls_ca_path: Some("./certs/ca.pem".to_string()),
        nats_mtls_client_cert_path: Some("./certs/client.pem".to_string()),
        nats_mtls_client_key_path: Some("./certs/client.key".to_string()),
        ..AetherConfig::default()
    });

    let channel = Channel::new("tls.demo");
    let mut receiver = Vibrator::create("tls-receiver", &aether).await;
    receiver.resonate_on(channel.clone()).await;

    let sender = Vibrator::create("tls-sender", &aether).await;
    sender
        .emit_wave(channel.clone(), serde_json::json!({"msg": "tls-ok"}))
        .await?;

    let wave = timeout(Duration::from_secs(1), receiver.receive()).await?
        .ok_or_else(|| anyhow::anyhow!("no wave received"))?;

    println!("TLS OK: {}", wave.payload());

    Ok(())
}
