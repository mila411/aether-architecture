//! Vibrator - a vibrating entity on the Aether layer (microservice)

use crate::{aether::Aether, channel::Channel, wave::Wave, Result};
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
}

impl VibratorConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            resonant_channels: Vec::new(),
            buffer_size: 100,
        }
    }

    pub fn with_channels(mut self, channels: Vec<Channel>) -> Self {
        self.resonant_channels = channels;
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

    /// Emit a wave (send a message)
    pub async fn emit(&self, wave: Wave) -> Result<()> {
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
                match receiver.recv().await {
                    Ok(wave) => return Some(wave),
                    Err(_) => return None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vibrator_creation() {
        let aether = Aether::default();
        let config = VibratorConfig::new("test-vibrator");
        let vibrator = Vibrator::new(config, &aether).await;

        assert_eq!(vibrator.name(), "test-vibrator");
    }

    #[tokio::test]
    async fn test_vibrator_emit_and_receive() {
        let aether = Aether::default();
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
}
