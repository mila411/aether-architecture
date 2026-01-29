//! Configuration management for Aether services

use crate::aether::AetherConfig;
use config::{Config, Environment, File};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{debug, info, warn};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub service: ServiceConfig,
    #[serde(default)]
    pub aether: AetherLayerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub operations: OperationsConfig,
    #[serde(default)]
    pub resource_monitoring: ResourceMonitoringConfig,
}

impl AppConfig {
    fn apply_service_name(&mut self, service_name: &str) {
        if self.service.name.trim().is_empty() {
            self.service.name = service_name.to_string();
        }
    }

    pub fn aether_config(&self) -> AetherConfig {
        self.aether.clone().into()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig::default(),
            aether: AetherLayerConfig::default(),
            logging: LoggingConfig::default(),
            observability: ObservabilityConfig::default(),
            operations: OperationsConfig::default(),
            resource_monitoring: ResourceMonitoringConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default = "default_max_inflight")]
    pub max_inflight: usize,
    #[serde(default)]
    pub rate_limit_per_sec: Option<f64>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_retry_max")]
    pub retry_max: usize,
    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,
    #[serde(default = "default_retry_max_delay_ms")]
    pub retry_max_delay_ms: u64,
    #[serde(default = "default_circuit_failure_threshold")]
    pub circuit_breaker_failure_threshold: usize,
    #[serde(default = "default_circuit_open_ms")]
    pub circuit_breaker_open_ms: u64,
    #[serde(default = "default_circuit_half_open_successes")]
    pub circuit_breaker_half_open_successes: usize,
    #[serde(default = "default_noise_floor")]
    pub noise_floor: f64,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            channels: Vec::new(),
            max_inflight: default_max_inflight(),
            rate_limit_per_sec: None,
            timeout_ms: default_timeout_ms(),
            retry_max: default_retry_max(),
            retry_base_delay_ms: default_retry_base_delay_ms(),
            retry_max_delay_ms: default_retry_max_delay_ms(),
            circuit_breaker_failure_threshold: default_circuit_failure_threshold(),
            circuit_breaker_open_ms: default_circuit_open_ms(),
            circuit_breaker_half_open_successes: default_circuit_half_open_successes(),
            noise_floor: default_noise_floor(),
        }
    }
}

fn default_max_inflight() -> usize {
    100
}

fn default_timeout_ms() -> u64 {
    2_000
}

fn default_retry_max() -> usize {
    3
}

fn default_retry_base_delay_ms() -> u64 {
    50
}

fn default_retry_max_delay_ms() -> u64 {
    500
}

fn default_circuit_failure_threshold() -> usize {
    5
}

fn default_circuit_open_ms() -> u64 {
    10_000
}

fn default_circuit_half_open_successes() -> usize {
    2
}

fn default_noise_floor() -> f64 {
    0.01
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default = "default_log_json")]
    pub log_json: bool,
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    #[serde(default = "default_metrics_bind")]
    pub metrics_bind: String,
    #[serde(default)]
    pub otlp_endpoint: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_json: default_log_json(),
            metrics_enabled: default_metrics_enabled(),
            metrics_bind: default_metrics_bind(),
            otlp_endpoint: None,
        }
    }
}

fn default_log_json() -> bool {
    false
}

fn default_metrics_enabled() -> bool {
    false
}

fn default_metrics_bind() -> String {
    "127.0.0.1:9000".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct OperationsConfig {
    #[serde(default = "default_health_enabled")]
    pub health_enabled: bool,
    #[serde(default = "default_health_bind")]
    pub health_bind: String,
    #[serde(default = "default_shutdown_grace_ms")]
    pub shutdown_grace_ms: u64,
    #[serde(default)]
    pub memory_limit_bytes: Option<u64>,
    #[serde(default)]
    pub cpu_time_limit_secs: Option<u64>,
}

impl Default for OperationsConfig {
    fn default() -> Self {
        Self {
            health_enabled: default_health_enabled(),
            health_bind: default_health_bind(),
            shutdown_grace_ms: default_shutdown_grace_ms(),
            memory_limit_bytes: None,
            cpu_time_limit_secs: None,
        }
    }
}

fn default_health_enabled() -> bool {
    true
}

fn default_health_bind() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_shutdown_grace_ms() -> u64 {
    5000
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceMonitoringConfig {
    #[serde(default = "default_resource_monitor_enabled")]
    pub enabled: bool,
    #[serde(default = "default_resource_monitor_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_leak_detection_enabled")]
    pub leak_detection_enabled: bool,
    #[serde(default = "default_leak_growth_bytes_per_min")]
    pub leak_growth_bytes_per_min: u64,
    #[serde(default = "default_allocator_metrics_enabled")]
    pub allocator_metrics_enabled: bool,
}

impl Default for ResourceMonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: default_resource_monitor_enabled(),
            interval_ms: default_resource_monitor_interval_ms(),
            leak_detection_enabled: default_leak_detection_enabled(),
            leak_growth_bytes_per_min: default_leak_growth_bytes_per_min(),
            allocator_metrics_enabled: default_allocator_metrics_enabled(),
        }
    }
}

fn default_resource_monitor_enabled() -> bool {
    true
}

fn default_resource_monitor_interval_ms() -> u64 {
    1000
}

fn default_leak_detection_enabled() -> bool {
    false
}

fn default_leak_growth_bytes_per_min() -> u64 {
    10 * 1024 * 1024
}

fn default_allocator_metrics_enabled() -> bool {
    false
}

#[derive(Debug, Clone, Deserialize)]
pub struct AetherLayerConfig {
    #[serde(default = "default_channel_buffer_size")]
    pub channel_buffer_size: usize,
    #[serde(default = "default_max_propagation")]
    pub max_propagation: u32,
    #[serde(default = "default_attenuation_factor")]
    pub attenuation_factor: f64,
    #[serde(default = "default_min_amplitude")]
    pub min_amplitude: f64,
    #[serde(default = "default_enable_physics")]
    pub enable_physics: bool,
    #[serde(default = "default_use_nats")]
    pub use_nats: bool,
    #[serde(default = "default_nats_url")]
    pub nats_url: String,

    #[serde(default = "default_nats_tls_required")]
    pub nats_tls_required: bool,

    #[serde(default)]
    pub auth_token: Option<String>,

    #[serde(default)]
    pub allowed_sources: Vec<String>,

    #[serde(default = "default_max_payload_bytes")]
    pub max_payload_bytes: usize,

    #[serde(default = "default_max_channel_length")]
    pub max_channel_length: usize,

    #[serde(default)]
    pub nats_mtls_ca_path: Option<String>,
    #[serde(default)]
    pub nats_mtls_client_cert_path: Option<String>,
    #[serde(default)]
    pub nats_mtls_client_key_path: Option<String>,

    #[serde(default = "default_persistence_enabled")]
    pub persistence_enabled: bool,
    #[serde(default = "default_persistence_path")]
    pub persistence_path: String,
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval: u64,
}

impl Default for AetherLayerConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: default_channel_buffer_size(),
            max_propagation: default_max_propagation(),
            attenuation_factor: default_attenuation_factor(),
            min_amplitude: default_min_amplitude(),
            enable_physics: default_enable_physics(),
            use_nats: default_use_nats(),
            nats_url: default_nats_url(),
            nats_tls_required: default_nats_tls_required(),
            auth_token: None,
            allowed_sources: Vec::new(),
            max_payload_bytes: default_max_payload_bytes(),
            max_channel_length: default_max_channel_length(),
            nats_mtls_ca_path: None,
            nats_mtls_client_cert_path: None,
            nats_mtls_client_key_path: None,
            persistence_enabled: default_persistence_enabled(),
            persistence_path: default_persistence_path(),
            snapshot_interval: default_snapshot_interval(),
        }
    }
}

impl From<AetherLayerConfig> for AetherConfig {
    fn from(config: AetherLayerConfig) -> Self {
        Self {
            channel_buffer_size: config.channel_buffer_size,
            max_propagation: config.max_propagation,
            attenuation_factor: config.attenuation_factor,
            min_amplitude: config.min_amplitude,
            enable_physics: config.enable_physics,
            use_nats: config.use_nats,
            nats_url: config.nats_url,
            nats_tls_required: config.nats_tls_required,
            auth_token: config.auth_token,
            allowed_sources: config.allowed_sources,
            max_payload_bytes: config.max_payload_bytes,
            max_channel_length: config.max_channel_length,
            nats_mtls_ca_path: config.nats_mtls_ca_path,
            nats_mtls_client_cert_path: config.nats_mtls_client_cert_path,
            nats_mtls_client_key_path: config.nats_mtls_client_key_path,
            persistence_enabled: config.persistence_enabled,
            persistence_path: config.persistence_path,
            snapshot_interval: config.snapshot_interval,
        }
    }
}

fn default_channel_buffer_size() -> usize {
    1000
}

fn default_max_propagation() -> u32 {
    10
}

fn default_attenuation_factor() -> f64 {
    0.95
}

fn default_min_amplitude() -> f64 {
    0.01
}

fn default_enable_physics() -> bool {
    true
}

fn default_use_nats() -> bool {
    true
}

fn default_nats_url() -> String {
    "nats://127.0.0.1:4222".to_string()
}

fn default_nats_tls_required() -> bool {
    false
}

fn default_max_payload_bytes() -> usize {
    1024 * 1024
}

fn default_max_channel_length() -> usize {
    128
}

fn default_persistence_enabled() -> bool {
    false
}

fn default_persistence_path() -> String {
    "./data/aether".to_string()
}

fn default_snapshot_interval() -> u64 {
    1000
}

pub fn load_config(service_name: &str) -> ConfigResult<AppConfig> {
    let paths = config_paths(service_name);
    load_config_from_paths(service_name, &paths)
}

pub fn watch_config(service_name: &str) -> ConfigResult<watch::Receiver<AppConfig>> {
    let initial = load_config(service_name)?;
    let (sender, receiver) = watch::channel(initial.clone());

    let service = service_name.to_string();
    let watch_path = select_watch_path(&config_paths(&service));

    if let Some(path) = watch_path {
        std::thread::spawn(move || {
            if let Err(err) = watch_config_file(&service, path, sender) {
                warn!("Config watch stopped: {}", err);
            }
        });
    } else {
        warn!(
            "No config file found to watch for service {}. Dynamic reload disabled.",
            service
        );
    }

    Ok(receiver)
}

fn load_config_from_paths(service_name: &str, paths: &[PathBuf]) -> ConfigResult<AppConfig> {
    let mut builder = Config::builder();

    for path in paths {
        builder = builder.add_source(File::from(path.as_path()).required(false));
    }

    builder = builder.add_source(
        Environment::with_prefix("AETHER")
            .separator("__")
            .try_parsing(true),
    );

    let settings = builder.build()?;
    let mut config: AppConfig = settings.try_deserialize()?;
    config.apply_service_name(service_name);
    Ok(config)
}

fn config_paths(service_name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let env = std::env::var("AETHER_ENV").unwrap_or_else(|_| "development".to_string());

    paths.push(PathBuf::from("config/default.toml"));
    paths.push(PathBuf::from(format!("config/{}.toml", env)));
    paths.push(PathBuf::from(format!("config/{}.toml", service_name)));

    if let Ok(explicit) = std::env::var("AETHER_CONFIG") {
        paths.push(PathBuf::from(explicit));
    }

    paths
}

fn select_watch_path(paths: &[PathBuf]) -> Option<PathBuf> {
    for path in paths.iter().rev() {
        if path.exists() {
            return Some(path.clone());
        }
    }
    None
}

fn watch_config_file(
    service_name: &str,
    path: PathBuf,
    sender: watch::Sender<AppConfig>,
) -> ConfigResult<()> {
    let (notify_tx, notify_rx) = channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = notify_tx.send(res);
    })
    .map_err(|e| config::ConfigError::Message(e.to_string()))?;

    watcher
        .watch(path.as_path(), RecursiveMode::NonRecursive)
        .map_err(|e| config::ConfigError::Message(e.to_string()))?;

    info!("Watching config file for {}: {:?}", service_name, path);

    loop {
        match notify_rx.recv() {
            Ok(Ok(event)) => {
                if !should_reload(&event.kind) {
                    continue;
                }

                debug!("Config change detected: {:?}", event.kind);
                std::thread::sleep(Duration::from_millis(200));

                match load_config(service_name) {
                    Ok(new_config) => {
                        let _ = sender.send(new_config);
                    }
                    Err(err) => {
                        warn!("Failed to reload config: {}", err);
                    }
                }
            }
            Ok(Err(err)) => {
                warn!("Config watch error: {}", err);
            }
            Err(_) => break,
        }
    }

    Ok(())
}

fn should_reload(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    )
}

pub fn config_path_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}
