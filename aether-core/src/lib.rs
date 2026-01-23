//! Aether Architecture Core Library
//!
//! A microservice framework applying aether theory to system architecture

pub mod aether;
pub mod channel;
pub mod physics;
pub mod vibrator;
pub mod wave;

pub use aether::{Aether, AetherConfig, AetherStats};
pub use channel::Channel;
pub use physics::{Interference, PhysicsEngine, Resonance};
pub use vibrator::{Vibrator, VibratorConfig};
pub use wave::{Amplitude, Wave, WaveType};

/// Error type for the Aether architecture
#[derive(Debug, thiserror::Error)]
pub enum AetherError {
    #[error("Failed to connect to Aether layer: {0}")]
    ConnectionFailed(String),

    #[error("Failed to send wave: {0}")]
    TransmissionFailed(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Invalid vibrator: {0}")]
    InvalidVibrator(String),

    #[error("Physics computation error: {0}")]
    PhysicsError(String),
}

pub type Result<T> = std::result::Result<T, AetherError>;

#[cfg(test)]
mod tests {
    #[test]
    fn test_core_exports() {
        // Ensure core modules are exported
    }
}
