//! Aether - Aether layer implementation

use crate::{channel::Channel, wave::Wave, AetherError, Result};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock};
use tracing::{debug, info, warn};

/// Aether layer configuration
#[derive(Debug, Clone)]
pub struct AetherConfig {
    /// Buffer size per channel
    pub channel_buffer_size: usize,

    /// Maximum propagation count for waves
    pub max_propagation: u32,

    /// Attenuation factor
    pub attenuation_factor: f64,

    /// Enable physics engine
    pub enable_physics: bool,

    /// Use NATS as the transport backend
    pub use_nats: bool,

    /// NATS server URL
    pub nats_url: String,
}

impl Default for AetherConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: 1000,
            max_propagation: 10,
            attenuation_factor: 0.95,
            enable_physics: true,
            use_nats: true,
            nats_url: "nats://127.0.0.1:4222".to_string(),
        }
    }
}

/// Aether layer - communication medium encompassing all services
pub struct Aether {
    /// Configuration
    config: AetherConfig,

    /// Broadcast channels per channel
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<Wave>>>>,

    /// Statistics
    stats: Arc<RwLock<AetherStats>>,

    /// NATS client
    nats_client: Arc<OnceCell<async_nats::Client>>,
}

/// Aether layer statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct AetherStats {
    pub total_waves: u64,
    pub active_channels: usize,
    pub total_vibrators: usize,
}

impl Aether {
    /// Create a new Aether layer
    pub fn new(config: AetherConfig) -> Self {
        info!("Initializing Aether layer...");
        Self {
            config,
            channels: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(AetherStats::default())),
            nats_client: Arc::new(OnceCell::new()),
        }
    }

    /// Create an Aether layer with default configuration
    pub fn default() -> Self {
        Self::new(AetherConfig::default())
    }

    /// Emit a wave into the Aether layer
    pub async fn emit(&self, mut wave: Wave) -> Result<()> {
        // Check propagation count
        if wave.propagation_count() >= self.config.max_propagation {
            warn!("Wave {} reached max propagation count", wave.id());
            return Ok(());
        }

        // Validity check
        if !wave.is_valid() {
            debug!("Skipping invalid wave {}", wave.id());
            return Ok(());
        }

        wave.propagate();

        let channel_name = wave.channel().name().to_string();

        if self.config.use_nats {
            let subject = nats_subject(&channel_name);
            let payload = serde_json::to_vec(&wave)
                .map_err(|e| AetherError::TransmissionFailed(e.to_string()))?;
            let client = self.nats_client().await?;

            if let Err(e) = client.publish(subject, payload.into()).await {
                return Err(AetherError::TransmissionFailed(e.to_string()));
            }

            // Update statistics
            let mut stats = self.stats.write().await;
            stats.total_waves += 1;

            debug!("Published wave {} to NATS", wave.id());
            return Ok(());
        }

        // Create channel if it does not exist
        let sender = {
            let mut channels = self.channels.write().await;
            channels
                .entry(channel_name.clone())
                .or_insert_with(|| {
                    debug!("Creating new channel: {}", channel_name);
                    let (tx, _) = broadcast::channel(self.config.channel_buffer_size);
                    tx
                })
                .clone()
        };

        // Send wave
        match sender.send(wave.clone()) {
            Ok(receiver_count) => {
                debug!(
                    "Sent wave {} to channel {} ({} receivers)",
                    wave.id(),
                    channel_name,
                    receiver_count
                );

                // Update statistics
                let mut stats = self.stats.write().await;
                stats.total_waves += 1;
            }
            Err(e) => {
                warn!("Failed to send wave: {:?}", e);
            }
        }

        Ok(())
    }

    /// Get a receiver to listen on a specific channel
    pub async fn subscribe(&self, channel: &Channel) -> broadcast::Receiver<Wave> {
        let channel_name = channel.name().to_string();

        let mut channels = self.channels.write().await;
        let mut created = false;
        let sender = channels
            .entry(channel_name.clone())
            .or_insert_with(|| {
                created = true;
                debug!("Creating channel {} (subscribe)", channel_name);
                let (tx, _) = broadcast::channel(self.config.channel_buffer_size);
                tx
            })
            .clone();

        if self.config.use_nats && created {
            let subject = nats_subject(&channel_name);
            let sender_clone = sender.clone();
            let client_result = self.nats_client().await;

            match client_result {
                Ok(client) => {
                    tokio::spawn(async move {
                        let subject_for_log = subject.clone();
                        match client.subscribe(subject).await {
                            Ok(mut subscriber) => {
                                while let Some(message) = subscriber.next().await {
                                    match serde_json::from_slice::<Wave>(&message.payload) {
                                        Ok(wave) => {
                                            let _ = sender_clone.send(wave);
                                        }
                                        Err(err) => {
                                            warn!("Failed to decode wave from NATS: {}", err);
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                warn!(
                                    "Failed to subscribe to NATS subject {}: {}",
                                    subject_for_log, err
                                );
                            }
                        }
                    });
                }
                Err(err) => {
                    warn!("Failed to connect to NATS: {}", err);
                }
            }
        }

        sender.subscribe()
    }

    /// Listen on multiple channels
    pub async fn subscribe_many(&self, channels: Vec<Channel>) -> Vec<broadcast::Receiver<Wave>> {
        let mut receivers = Vec::new();
        for channel in channels {
            receivers.push(self.subscribe(&channel).await);
        }
        receivers
    }

    /// Get Aether layer statistics
    pub async fn stats(&self) -> AetherStats {
        let stats = self.stats.read().await;
        let channels = self.channels.read().await;

        AetherStats {
            total_waves: stats.total_waves,
            active_channels: channels.len(),
            total_vibrators: stats.total_vibrators,
        }
    }

    /// Get list of active channels
    pub async fn active_channels(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }

    /// Remove a specific channel (cleanup)
    pub async fn remove_channel(&self, channel: &Channel) -> Result<()> {
        let channel_name = channel.name();
        let mut channels = self.channels.write().await;

        if channels.remove(channel_name).is_some() {
            info!("Removed channel {}", channel_name);
            Ok(())
        } else {
            Err(AetherError::ChannelNotFound(channel_name.to_string()))
        }
    }

    /// Clear the Aether layer
    pub async fn clear(&self) {
        let mut channels = self.channels.write().await;
        channels.clear();
        info!("Cleared the Aether layer");
    }

    /// Get configuration
    pub fn config(&self) -> &AetherConfig {
        &self.config
    }

    async fn nats_client(&self) -> Result<async_nats::Client> {
        let url = self.config.nats_url.clone();
        let client = self
            .nats_client
            .get_or_try_init(|| async move {
                async_nats::connect(url)
                    .await
                    .map_err(|e| AetherError::ConnectionFailed(e.to_string()))
            })
            .await?;
        Ok(client.clone())
    }
}

impl Clone for Aether {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            channels: Arc::clone(&self.channels),
            stats: Arc::clone(&self.stats),
            nats_client: Arc::clone(&self.nats_client),
        }
    }
}

fn nats_subject(channel_name: &str) -> String {
    if channel_name == "*" {
        ">".to_string()
    } else {
        channel_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wave::WaveType;

    #[tokio::test]
    async fn test_aether_creation() {
        let aether = Aether::new(AetherConfig {
            use_nats: false,
            ..AetherConfig::default()
        });
        let stats = aether.stats().await;
        assert_eq!(stats.active_channels, 0);
    }

    #[tokio::test]
    async fn test_emit_and_subscribe() {
        let aether = Aether::new(AetherConfig {
            use_nats: false,
            ..AetherConfig::default()
        });
        let channel = Channel::new("test.channel");

        let mut receiver = aether.subscribe(&channel).await;

        let wave = Wave::builder(channel.clone())
            .payload(serde_json::json!({"message": "hello"}))
            .build();

        aether.emit(wave.clone()).await.unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.channel().name(), channel.name());
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let aether = Aether::new(AetherConfig {
            use_nats: false,
            ..AetherConfig::default()
        });
        let channel = Channel::new("broadcast");

        let mut rx1 = aether.subscribe(&channel).await;
        let mut rx2 = aether.subscribe(&channel).await;

        let wave = Wave::builder(channel.clone())
            .wave_type(WaveType::Broadcast)
            .payload(serde_json::json!({"data": "test"}))
            .build();

        aether.emit(wave).await.unwrap();

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }
}
