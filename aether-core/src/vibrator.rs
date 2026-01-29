//! Vibrator - a vibrating entity on the Aether layer (microservice)

use crate::{aether::Aether, channel::Channel, wave::Wave, Result};
use bytes::Bytes;
use tokio::sync::broadcast;
use tracing::{debug, info};

/// Vibrator configuration
#[derive(Debug, Clone)]
pub struct VibratorConfig {
    /// Vibrator name
    pub name: String,

    /// Channels to resonate with automatically
    pub resonant_channels: Vec<Channel>,

    /// Receive buffer size
    pub buffer_size: usize,

    /// Auth token for wave authentication
    pub auth_token: Option<String>,

    /// Noise floor (waves below this amplitude are ignored)
    pub noise_floor: f64,
}

impl VibratorConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            resonant_channels: Vec::new(),
            buffer_size: 100,
            auth_token: None,
            noise_floor: 0.01,
        }
    }

    pub fn with_channels(mut self, channels: Vec<Channel>) -> Self {
        self.resonant_channels = channels;
        self
    }

    pub fn with_auth_token(mut self, token: Option<String>) -> Self {
        self.auth_token = token;
        self
    }

    pub fn with_noise_floor(mut self, noise_floor: f64) -> Self {
        self.noise_floor = noise_floor;
        self
    }
}

/// Vibrator - a service that vibrates on the Aether layer
///
/// Each microservice operates as a vibrator,
/// resonates at specific frequencies (channels) to receive messages,
/// and emits waves to send messages.
pub struct Vibrator {
    /// Vibrator configuration
    config: VibratorConfig,

    /// Reference to the Aether layer
    aether: Aether,

    /// Receivers for resonant channels
    receivers: Vec<(Channel, broadcast::Receiver<Wave>)>,
}

/// Lightweight emitter handle for concurrent tasks
#[derive(Clone)]
pub struct VibratorEmitter {
    name: String,
    aether: Aether,
    auth_token: Option<String>,
}

impl Vibrator {
    /// Create a new vibrator
    pub async fn new(config: VibratorConfig, aether: &Aether) -> Self {
        info!("Initializing vibrator {}...", config.name);

        let mut vibrator = Self {
            config,
            aether: aether.clone(),
            receivers: Vec::new(),
        };

        // Set initial resonant channels
        let channels = vibrator.config.resonant_channels.clone();
        for channel in channels {
            vibrator.resonate_on(channel).await;
        }

        vibrator
    }

    /// Simple constructor
    pub async fn create(name: impl Into<String>, aether: &Aether) -> Self {
        Self::new(VibratorConfig::new(name), aether).await
    }

    /// Start resonating on a specific channel (start listening)
    pub async fn resonate_on(&mut self, channel: Channel) {
        debug!(
            "Vibrator {} started resonating on channel {}",
            self.config.name, channel
        );

        let receiver = self.aether.subscribe(&channel).await;
        self.receivers.push((channel, receiver));
    }

    /// Resonates on multiple channels
    pub async fn resonate_on_many(&mut self, channels: Vec<Channel>) {
        for channel in channels {
            self.resonate_on(channel).await;
        }
    }

    /// Resonate on a frequency hopping set derived from a base channel
    pub async fn resonate_hopping(&mut self, base: Channel, hop_count: u16) {
        let hop_channels = base.hop_set(hop_count);
        self.resonate_on_many(hop_channels).await;
    }

    /// Emit a wave (send a message)
    pub async fn emit(&self, mut wave: Wave) -> Result<()> {
        if let Some(token) = &self.config.auth_token {
            wave.set_auth_token(token.clone());
        }
        debug!("Vibrator {} emitted wave {}", self.config.name, wave.id());
        self.aether.emit(wave).await
    }

    /// Build and emit a wave
    pub async fn emit_wave(
        &self,
        channel: impl Into<Channel>,
        payload: serde_json::Value,
    ) -> Result<()> {
        let wave = Wave::builder(channel)
            .payload(payload)
            .source(self.config.name.clone())
            .build();

        self.emit(wave).await
    }

    /// Build and emit a frequency-hopped wave
    pub async fn emit_hopping_wave(
        &self,
        base_channel: impl Into<Channel>,
        hop_index: u16,
        hop_count: u16,
        payload: serde_json::Value,
    ) -> Result<()> {
        let channel = base_channel.into().hop(hop_index, hop_count);
        self.emit_wave(channel, payload).await
    }

    /// Build and emit a time-synchronized frequency-hopped wave
    pub async fn emit_time_hopping_wave(
        &self,
        base_channel: impl Into<Channel>,
        hop_count: u16,
        hop_interval_ms: u64,
        payload: serde_json::Value,
    ) -> Result<()> {
        let base = base_channel.into();
        let channel = base.hop_now(hop_count, hop_interval_ms);
        self.emit_wave(channel, payload).await
    }

    /// Build and emit a wave with raw bytes payload (zero-copy)
    pub async fn emit_bytes(&self, channel: impl Into<Channel>, payload: Bytes) -> Result<()> {
        let wave = Wave::builder(channel)
            .payload_bytes(payload)
            .source(self.config.name.clone())
            .build();

        self.emit(wave).await
    }

    /// Create a lightweight emitter handle for concurrent tasks
    pub fn emitter(&self) -> VibratorEmitter {
        VibratorEmitter {
            name: self.config.name.clone(),
            aether: self.aether.clone(),
            auth_token: self.config.auth_token.clone(),
        }
    }

    /// Receive the next wave (from any channel)
    pub async fn receive(&mut self) -> Option<Wave> {
        if self.receivers.is_empty() {
            return None;
        }

        // Try non-blocking receive from all receivers
        loop {
            for (channel, receiver) in &mut self.receivers {
                match receiver.try_recv() {
                    Ok(wave) => {
                        // Optionally ignore waves sent by self
                        if let Some(source) = wave.source() {
                            if source == self.config.name {
                                continue;
                            }
                        }

                        if wave.amplitude().value() < self.config.noise_floor {
                            continue;
                        }

                        debug!(
                            "Vibrator {} received wave {} from channel {}",
                            self.config.name,
                            wave.id(),
                            channel
                        );
                        return Some(wave);
                    }
                    Err(broadcast::error::TryRecvError::Empty) => continue,
                    Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                        debug!("Vibrator {} missed {} waves", self.config.name, skipped);
                        continue;
                    }
                    Err(broadcast::error::TryRecvError::Closed) => {
                        debug!("Channel {} was closed", channel);
                        continue;
                    }
                }
            }

            // If all receivers are empty, wait briefly
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    /// Receive only from a specific channel
    pub async fn receive_from(&mut self, channel: &Channel) -> Option<Wave> {
        for (ch, receiver) in &mut self.receivers {
            if ch == channel {
                loop {
                    match receiver.recv().await {
                        Ok(wave) => {
                            if let Some(source) = wave.source() {
                                if source == self.config.name {
                                    continue;
                                }
                            }
                            if wave.amplitude().value() < self.config.noise_floor {
                                continue;
                            }
                            return Some(wave);
                        }
                        Err(_) => return None,
                    }
                }
            }
        }
        None
    }

    /// Get vibrator name
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get list of resonant channels
    pub fn resonant_channels(&self) -> Vec<Channel> {
        self.receivers.iter().map(|(ch, _)| ch.clone()).collect()
    }
}

impl VibratorEmitter {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn emit(&self, wave: Wave) -> Result<()> {
        let mut wave = wave;
        if let Some(token) = &self.auth_token {
            wave.set_auth_token(token.clone());
        }
        self.aether.emit(wave).await
    }

    pub async fn emit_wave(
        &self,
        channel: impl Into<Channel>,
        payload: serde_json::Value,
    ) -> Result<()> {
        let wave = Wave::builder(channel)
            .payload(payload)
            .source(self.name.clone())
            .build();

        self.emit(wave).await
    }

    pub async fn emit_hopping_wave(
        &self,
        base_channel: impl Into<Channel>,
        hop_index: u16,
        hop_count: u16,
        payload: serde_json::Value,
    ) -> Result<()> {
        let channel = base_channel.into().hop(hop_index, hop_count);
        self.emit_wave(channel, payload).await
    }

    pub async fn emit_time_hopping_wave(
        &self,
        base_channel: impl Into<Channel>,
        hop_count: u16,
        hop_interval_ms: u64,
        payload: serde_json::Value,
    ) -> Result<()> {
        let base = base_channel.into();
        let channel = base.hop_now(hop_count, hop_interval_ms);
        self.emit_wave(channel, payload).await
    }

    pub async fn emit_bytes(&self, channel: impl Into<Channel>, payload: Bytes) -> Result<()> {
        let wave = Wave::builder(channel)
            .payload_bytes(payload)
            .source(self.name.clone())
            .build();

        self.emit(wave).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aether::AetherConfig;
    use tokio::time::{timeout, Duration};

    fn test_aether() -> Aether {
        Aether::new(AetherConfig {
            use_nats: false,
            ..AetherConfig::default()
        })
    }

    #[tokio::test]
    async fn test_vibrator_creation() {
        let aether = test_aether();
        let config = VibratorConfig::new("test-vibrator");
        let vibrator = Vibrator::new(config, &aether).await;

        assert_eq!(vibrator.name(), "test-vibrator");
    }

    #[tokio::test]
    async fn test_vibrator_emit_and_receive() {
        let aether = test_aether();
        let channel = Channel::new("test.communication");

        let mut vibrator1 = Vibrator::create("vibrator-1", &aether).await;
        let mut vibrator2 = Vibrator::create("vibrator-2", &aether).await;

        vibrator1.resonate_on(channel.clone()).await;
        vibrator2.resonate_on(channel.clone()).await;

        // vibrator1 sends
        vibrator1
            .emit_wave(channel.clone(), serde_json::json!({"msg": "hello"}))
            .await
            .unwrap();

        // Wait briefly
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // vibrator2 receives
        let wave = vibrator2.receive_from(&channel).await;
        assert!(wave.is_some());
    }

    #[tokio::test]
    async fn test_vibrator_noise_floor_filters_low_amplitude() {
        let aether = test_aether();
        let channel = Channel::new("noise.floor");

        let mut receiver = Vibrator::new(
            VibratorConfig::new("receiver").with_noise_floor(0.5),
            &aether,
        )
        .await;
        let sender = Vibrator::create("sender", &aether).await;

        receiver.resonate_on(channel.clone()).await;

        let low_wave = Wave::builder(channel.clone())
            .payload(serde_json::json!({"msg": "low"}))
            .amplitude(0.1)
            .build();

        sender.emit(low_wave).await.unwrap();

        let result = timeout(Duration::from_millis(50), receiver.receive()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_vibrator_time_hopping_emit_is_received() {
        let aether = test_aether();
        let base = Channel::new("orders");
        let hop_count = 4;
        let hop_interval_ms = 50;

        let mut receiver = Vibrator::create("receiver", &aether).await;
        receiver.resonate_hopping(base.clone(), hop_count).await;

        let sender = Vibrator::create("sender", &aether).await;
        sender
            .emit_time_hopping_wave(
                base.clone(),
                hop_count,
                hop_interval_ms,
                serde_json::json!({"msg": "hop"}),
            )
            .await
            .unwrap();

        let wave = timeout(Duration::from_millis(100), receiver.receive())
            .await
            .ok()
            .flatten();

        assert!(wave.is_some());
        let wave = wave.unwrap();
        let hops = base.hop_set(hop_count);
        assert!(hops.iter().any(|h| h.name() == wave.channel().name()));
    }
}
