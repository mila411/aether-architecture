<p align="center">
  <img src=".github/image/logs.png" alt="logs" width=85%>
</p>

# Aether Architecture

> An innovative microservice framework applying 19th‑century aether theory to system architecture

## 🌌 Concept

**Aether theory** was once assumed to be the medium through which light and electromagnetic waves travel. Applying this theory to system architecture addresses challenges in traditional microservice architectures.

### Core concepts

1. **Aether Layer**
   - A common communication medium encompassing all services
   - Eliminates direct coupling between services
   - Messages propagate as “waves”

2. **Wave Propagation**
   - Messages propagate like waves to multiple services
   - Models physical phenomena such as attenuation, interference, and resonance
   - A new form of event‑driven architecture

3. **Aether Vibration**
   - Services function as “vibrators” on the aether
   - Send and receive messages at specific frequencies (channels)
   - Harmony between services via resonance

4. **Properties as a Medium**
   - No service discovery required
   - Connecting to the aether automatically connects to the whole
   - Achieves complete loose coupling

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Aether Layer                         │
│        (Universal Communication Medium)                 │
│                                                          │
│   ┌─────────┐      ┌─────────┐      ┌─────────┐       │
│   │ Service │◄────►│ Service │◄────►│ Service │       │
│   │  Alpha  │ wave │  Beta   │ wave │  Gamma  │       │
│   └─────────┘      └─────────┘      └─────────┘       │
│        ▲                                    ▲            │
│        │         Vibration & Resonance     │            │
│        └────────────────┬──────────────────┘            │
│                         │                                │
│                   ┌──────────┐                          │
│                   │ Gateway  │                          │
│                   └──────────┘                          │
└─────────────────────────────────────────────────────────┘
```

## 🚀 Features

- **Complete loose coupling**: Services only know the aether, not each other
- **Dynamic propagation**: Messages naturally travel like waves
- **Frequency‑based routing**: Message filtering by channel (frequency)
- **Time‑synchronized frequency hopping**: Channel changes by time slot for concealment and robustness
- **Noise floor filtering**: Drop low‑amplitude waves to hide services and reduce noise
- **Attenuation and re‑amplification**: Message lifetime control and resend control
- **Interference patterns**: Interactions among multiple messages
- **Resonance detection**: Automatic discovery of related services

## ✅ Production features (Rust)

- **Config management**: Environment overlays, typed config, hot reload
- **Observability**: Structured logs, Prometheus metrics, OTLP tracing
- **Error handling**: Contextual errors, recoverable/unrecoverable distinction
- **Backpressure**: Task management, channel capacity control, rate limiting
- **Performance**: Zero‑copy payloads (`bytes`), buffer pool
- **Reliability**: Retry/timeout/circuit breaker
- **Persistence**: Append‑only log + snapshot, restart recovery
- **Security**: TLS/mTLS, auth/allow‑list, input validation, secret handling
- **Operations**: Graceful shutdown, health checks, panic hook, resource limits
- **Testing**: Property tests, benchmarks, fault injection
- **Resource monitoring**: RSS/VMS, leak hints, allocator metrics

## 📦 Project structure

```
aether-architecture/
├── aether-core/           # Core implementation of the Aether layer
│   ├── src/
│   │   ├── lib.rs
│   │   ├── aether.rs      # Aether layer
│   │   ├── wave.rs        # Wave messages
│   │   ├── vibrator.rs    # Service vibrator
│   │   └── physics.rs     # Physics simulation
│   ├── benches/
│   │   └── aether_bench.rs
│   ├── examples/
│   │   └── hopping_demo.rs
│   ├── tests/
│   │   ├── fault_injection.rs
│   │   └── property_tests.rs
│   └── Cargo.toml
├── aether-service-alpha/  # Sample service A
├── aether-service-beta/   # Sample service B
├── aether-gateway/        # Aether gateway
├── config/                # Default configs
│   └── default.toml
└── Cargo.toml
```

## 🔧 Usage

### Initialize the Aether layer

```rust
use aether_core::{Aether, AetherConfig};

#[tokio::main]
async fn main() {
    let aether = Aether::new(AetherConfig::default());
}
```

### Create a service

```rust
use aether_core::{Vibrator, Wave, Channel};

let mut vibrator = Vibrator::new("service-alpha", &aether);

// Listen on a specific frequency (channel)
vibrator.resonate_on(Channel::new("orders")).await;

// Receive messages
while let Some(wave) = vibrator.receive().await {
    println!("Received wave: {:?}", wave);
}

// Send a message
vibrator.emit(Wave::new("order.created", payload)).await;
```

## 🎯 Comparison with traditional architectures

| Feature          | Traditional microservices | Aether architecture      |
| ---------------- | ------------------------- | ------------------------ |
| Service coupling | Service name/URL          | Via aether medium        |
| Discovery        | Required (Consul, etc.)   | Not required (automatic) |
| Messaging        | Point‑to‑Point            | Wave Propagation         |
| Routing          | Explicit configuration    | Frequency‑based          |
| Scalability      | Complex                   | Naturally scales         |

## 🌊 Core concept details

### Wave

Messages are represented as “waves” with the following properties:

- **Amplitude**: Message importance
- **Frequency**: Message category (channel)
- **Phase**: Message state
- **Attenuation**: Decay over time

### Vibrator

Each microservice works as a vibrator:

- Resonates at specific frequencies (message reception)
- Emits waves (message sending)
- Detects interference patterns

### Physics

Simulates real physical phenomena:

- **Interference**: Interactions between multiple waves
- **Resonance**: Amplification at the same frequency
- **Diffraction**: Obstacle avoidance
- **Reflection**: Wave return on errors

## 🛠️ Build and run

```bash
# Build
cargo build --release

# Start NATS (required for inter-process communication)
# Option A: local nats-server
# nats-server
# Option B: Docker
# docker run --rm -p 4222:4222 nats:2

# Start services
cargo run --bin aether-service-alpha
cargo run --bin aether-service-beta
cargo run --bin aether-gateway

# Test
cargo test
```

## 📚 Theoretical background

The aether theory was proposed in the 19th century and later rejected by the theory of relativity. However, its concept of an “all‑pervasive medium” helps solve the following challenges in distributed systems:

1. **Complexity of service meshes**: The Aether layer provides a single abstraction
2. **Dynamic topology**: Service additions and removals are handled naturally
3. **Event‑driven complexity**: An intuitive model via wave propagation

## 🎓 Applicability

- **IoT networks**: Autonomous communication between devices
- **Edge computing**: Dynamic node join/leave
- **Real‑time systems**: Low‑latency wave propagation
- **Distributed AI**: Collaborative learning between models


## Quickstart

### Prerequisites

- Rust 1.70+
- Cargo
- NATS server (for inter-process messaging)

### Installation

```bash
git clone <repository-url>
cd aether-architecture
cargo build --release
```

### Basic usage

#### 1. Initialize the Aether layer

```rust
use aether_core::Aether;

let aether = Aether::default();
```

#### 2. Create a service (vibrator)

```rust
use aether_core::{Vibrator, Channel};

let mut service = Vibrator::create("my-service", &aether).await;
service.resonate_on(Channel::new("events")).await;
```

#### 3. Send and receive messages

```rust
// Send
service.emit_wave(
    Channel::new("user.created"),
    serde_json::json!({"user_id": "123"})
).await?;

// Receive
while let Some(wave) = service.receive().await {
    println!("Received: {:?}", wave.payload());
}
```

#### 4. Time‑synchronized hopping & noise floor (quick demo)

Time‑based frequency hopping rotates the channel each time slot. The noise floor filters weak waves so only “visible” signals are processed.

```rust
use aether_core::{Vibrator, Channel, Wave};

let hop_count = 4;
let hop_interval_ms = 200;

let mut receiver = Vibrator::create("receiver", &aether).await;
receiver.resonate_hopping(Channel::new("orders"), hop_count).await;

let sender = Vibrator::create("sender", &aether).await;
sender
    .emit_time_hopping_wave(
        Channel::new("orders"),
        hop_count,
        hop_interval_ms,
        serde_json::json!({"msg": "hop"}),
    )
    .await?;
```

Optional config (examples):

```toml
[aether]
min_amplitude = 0.01

[service]
noise_floor = 0.05
```

### Run the samples

#### Terminal 1: Start NATS

```bash
nats-server
```

#### Terminal 2: Start Gateway

```bash
cargo run --bin gateway
```

#### Terminal 3: Start Service Beta

```bash
cargo run --bin service-beta
```

#### Terminal 4: Start Service Alpha

```bash
cargo run --bin service-alpha
```

When Service Alpha creates an order, waves propagate through the Aether layer,
Service Beta checks inventory and returns the result.
You can observe all waves in the Gateway.

### Run tests

```bash
cargo test
```

### Documentation

- [Architecture guide](./ARCHITECTURE.md) - Detailed implementation guide


## 📄 License

MIT License

## 🤝 Contributing

This project is an experimental concept implementation. Ideas and improvements are welcome!

---

*"Like an invisible medium filling the universe, the Aether layer connects all services"*
