use aether_core::{Aether, AetherConfig, Channel, Wave};

#[tokio::test]
async fn reject_oversized_payload() {
    let aether = Aether::new(AetherConfig {
        use_nats: false,
        max_payload_bytes: 8,
        ..AetherConfig::default()
    });

    let wave = Wave::builder(Channel::new("test.payload"))
        .payload(serde_json::json!({"data": "this-is-too-large"}))
        .build();

    let result = aether.emit(wave).await;
    assert!(result.is_err());
}
