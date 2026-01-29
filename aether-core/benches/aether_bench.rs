use aether_core::{Aether, AetherConfig, Channel, Wave};
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_emit(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let aether = Aether::new(AetherConfig {
        use_nats: false,
        ..AetherConfig::default()
    });

    c.bench_function("aether_emit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let wave = Wave::builder(Channel::new("bench.emit"))
                    .payload(serde_json::json!({"data": "x"}))
                    .build();
                let _ = aether.emit(wave).await;
            })
        })
    });
}

criterion_group!(benches, bench_emit);
criterion_main!(benches);
