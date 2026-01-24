//! Aether Architecture Core Library
//!
//! A microservice framework applying aether theory to system architecture

pub mod aether;
pub mod buffer_pool;
pub mod channel;
pub mod config;
pub mod observability;
pub mod operations;
pub mod persistence;
pub mod physics;
pub mod reliability;
pub mod resource_monitoring;
pub mod task_manager;
pub mod vibrator;
pub mod wave;

pub use aether::{Aether, AetherConfig, AetherStats};
pub use buffer_pool::{BytePool, PooledBytesMut};
pub use channel::Channel;
pub use config::{
    load_config, watch_config, AetherLayerConfig, AppConfig, ConfigError, LoggingConfig,
    ObservabilityConfig, ServiceConfig,
};
pub use observability::{init_observability, ObservabilityGuard};
pub use operations::{
    apply_resource_limits, init_ops, install_panic_hook, shutdown_signal, wait_for_shutdown,
    OpsConfig,
};
pub use persistence::{AetherSnapshot, WaveStore};
pub use physics::{Interference, PhysicsEngine, Resonance};
pub use reliability::{retry_with_timeout, CircuitBreaker, RetryPolicy};
pub use resource_monitoring::{start_resource_monitoring, ResourceMonitorConfig};
pub use task_manager::TaskManager;
pub use vibrator::{Vibrator, VibratorConfig, VibratorEmitter};
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

    #[error("Persistence error: {0}")]
    PersistenceError(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

impl AetherError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AetherError::ConnectionFailed(_) | AetherError::TransmissionFailed(_)
        )
    }
}

pub type Result<T> = std::result::Result<T, AetherError>;

#[cfg(test)]
mod tests {
    #[test]
    fn test_core_exports() {
        // Ensure core modules are exported
    }
}
