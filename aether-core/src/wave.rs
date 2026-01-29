//! Wave - wave message propagating through the Aether layer

use crate::channel::Channel;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wave amplitude (represents importance)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Amplitude(f64);

impl Amplitude {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn value(&self) -> f64 {
        self.0
    }

    /// Apply attenuation over time
    pub fn attenuate(&mut self, factor: f64) {
        self.0 *= factor.clamp(0.0, 1.0);
    }

    /// Amplify via resonance
    pub fn amplify(&mut self, factor: f64) {
        self.0 = (self.0 * factor).min(1.0);
    }
}

impl Default for Amplitude {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Wave type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WaveType {
    /// Event notification
    Event,
    /// Command execution request
    Command,
    /// Query (read request)
    Query,
    /// Response
    Response,
    /// Broadcast
    Broadcast,
}

/// Wave message propagating through the Aether layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    /// Schema version
    #[serde(default = "default_schema_version")]
    schema_version: u16,
    /// Wave unique identifier
    id: Uuid,

    /// Wave type
    wave_type: WaveType,

    /// Channel (frequency)
    channel: Channel,

    /// Payload
    payload: serde_json::Value,

    /// Raw bytes payload (zero-copy)
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_bytes: Option<Bytes>,

    /// Amplitude (importance)
    amplitude: Amplitude,

    /// Source vibrator ID
    source: Option<String>,

    /// Sent timestamp
    timestamp: DateTime<Utc>,

    /// Metadata
    #[serde(default)]
    metadata: serde_json::Value,

    /// Wave phase (current state)
    #[serde(default)]
    phase: f64,

    /// Propagation count (hop count)
    #[serde(default)]
    propagation_count: u32,
}

const DEFAULT_MIN_AMPLITUDE: f64 = 0.01;

impl Wave {
    /// Create a new wave
    pub fn new(channel: impl Into<Channel>, payload: serde_json::Value) -> Self {
        Self {
            schema_version: current_schema_version(),
            id: Uuid::new_v4(),
            wave_type: WaveType::Event,
            channel: channel.into(),
            payload,
            payload_bytes: None,
            amplitude: Amplitude::default(),
            source: None,
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
            phase: 0.0,
            propagation_count: 0,
        }
    }

    /// Create a new wave with raw bytes payload
    pub fn new_bytes(channel: impl Into<Channel>, payload: Bytes) -> Self {
        Self {
            schema_version: current_schema_version(),
            id: Uuid::new_v4(),
            wave_type: WaveType::Event,
            channel: channel.into(),
            payload: serde_json::Value::Null,
            payload_bytes: Some(payload),
            amplitude: Amplitude::default(),
            source: None,
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
            phase: 0.0,
            propagation_count: 0,
        }
    }

    /// Build a wave using the builder pattern
    pub fn builder(channel: impl Into<Channel>) -> WaveBuilder {
        WaveBuilder::new(channel)
    }

    // Getter methods
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn wave_type(&self) -> &WaveType {
        &self.wave_type
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn payload(&self) -> &serde_json::Value {
        &self.payload
    }

    pub fn payload_bytes(&self) -> Option<&Bytes> {
        self.payload_bytes.as_ref()
    }

    pub fn auth_token(&self) -> Option<&str> {
        self.metadata.get("auth_token").and_then(|v| v.as_str())
    }

    pub fn set_auth_token(&mut self, token: impl Into<String>) {
        let token = token.into();
        if let Some(obj) = self.metadata.as_object_mut() {
            obj.insert("auth_token".to_string(), serde_json::Value::String(token));
        } else {
            self.metadata = serde_json::json!({ "auth_token": token });
        }
    }

    pub fn amplitude(&self) -> &Amplitude {
        &self.amplitude
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub fn timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }

    pub fn propagation_count(&self) -> u32 {
        self.propagation_count
    }

    /// Schema compatibility check
    pub fn is_compatible(&self) -> bool {
        self.schema_version <= current_schema_version()
    }

    /// Propagate the wave (increment hop count)
    pub fn propagate(&mut self) {
        self.propagation_count += 1;
        // Attenuate on each propagation
        self.amplitude.attenuate(0.95);
        // Advance phase
        self.phase += std::f64::consts::PI / 4.0;
    }

    /// Whether the wave is valid (amplitude above threshold)
    pub fn is_valid(&self) -> bool {
        self.is_valid_with_threshold(DEFAULT_MIN_AMPLITUDE)
    }

    /// Whether the wave is valid (amplitude above a custom threshold)
    pub fn is_valid_with_threshold(&self, min_amplitude: f64) -> bool {
        self.amplitude.value() > min_amplitude
    }

    /// Apply attenuation over time
    pub fn apply_time_decay(&mut self) {
        let elapsed = Utc::now().signed_duration_since(self.timestamp);
        let seconds = elapsed.num_seconds() as f64;
        let decay_factor = (-seconds / 60.0).exp(); // Attenuate over 60 seconds
        self.amplitude.attenuate(decay_factor);
    }
}

/// Wave builder
pub struct WaveBuilder {
    channel: Channel,
    payload: Option<serde_json::Value>,
    payload_bytes: Option<Bytes>,
    wave_type: WaveType,
    amplitude: Amplitude,
    source: Option<String>,
    metadata: serde_json::Value,
    schema_version: u16,
}

impl WaveBuilder {
    pub fn new(channel: impl Into<Channel>) -> Self {
        Self {
            channel: channel.into(),
            payload: None,
            payload_bytes: None,
            wave_type: WaveType::Event,
            amplitude: Amplitude::default(),
            source: None,
            metadata: serde_json::json!({}),
            schema_version: current_schema_version(),
        }
    }

    pub fn payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn payload_bytes(mut self, payload: Bytes) -> Self {
        self.payload_bytes = Some(payload);
        self
    }

    pub fn wave_type(mut self, wave_type: WaveType) -> Self {
        self.wave_type = wave_type;
        self
    }

    pub fn amplitude(mut self, amplitude: f64) -> Self {
        self.amplitude = Amplitude::new(amplitude);
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn schema_version(mut self, version: u16) -> Self {
        self.schema_version = version;
        self
    }

    pub fn build(self) -> Wave {
        Wave {
            schema_version: self.schema_version,
            id: Uuid::new_v4(),
            wave_type: self.wave_type,
            channel: self.channel,
            payload: self.payload.unwrap_or_else(|| {
                if self.payload_bytes.is_some() {
                    serde_json::Value::Null
                } else {
                    serde_json::json!({})
                }
            }),
            payload_bytes: self.payload_bytes,
            amplitude: self.amplitude,
            source: self.source,
            timestamp: Utc::now(),
            metadata: self.metadata,
            phase: 0.0,
            propagation_count: 0,
        }
    }
}

fn current_schema_version() -> u16 {
    1
}

fn default_schema_version() -> u16 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_creation() {
        let wave = Wave::new("test.channel", serde_json::json!({"key": "value"}));
        assert_eq!(wave.channel().name(), "test.channel");
        assert!(wave.is_valid());
    }

    #[test]
    fn test_wave_propagation() {
        let mut wave = Wave::new("test", serde_json::json!({}));
        let initial_amplitude = wave.amplitude().value();

        wave.propagate();

        assert_eq!(wave.propagation_count(), 1);
        assert!(wave.amplitude().value() < initial_amplitude);
    }

    #[test]
    fn test_amplitude_attenuation() {
        let mut amplitude = Amplitude::new(1.0);
        amplitude.attenuate(0.5);
        assert_eq!(amplitude.value(), 0.5);
    }

    #[test]
    fn test_wave_builder() {
        let wave = Wave::builder("test.channel")
            .payload(serde_json::json!({"data": "test"}))
            .wave_type(WaveType::Command)
            .amplitude(0.8)
            .source("service-1")
            .build();

        assert_eq!(wave.channel().name(), "test.channel");
        assert_eq!(wave.wave_type(), &WaveType::Command);
        assert_eq!(wave.source(), Some("service-1"));
    }
}
