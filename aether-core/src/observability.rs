//! Observability utilities: logging, metrics, and tracing.

use crate::config::AppConfig;
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_exporter_prometheus::PrometheusHandle;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::trace as sdktrace;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use tracing_subscriber::layer::{Layer, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Debug)]
pub struct ObservabilityGuard {
    _metrics_task: Option<JoinHandle<()>>,
}

impl Drop for ObservabilityGuard {
    fn drop(&mut self) {
        opentelemetry::global::shutdown_tracer_provider();
    }
}

pub fn init_observability(config: &AppConfig) -> anyhow::Result<ObservabilityGuard> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(config.logging.level.clone()))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer: Box<dyn tracing_subscriber::Layer<_> + Send + Sync> = if config.observability.log_json {
        fmt::layer()
            .json()
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_filter(env_filter)
            .boxed()
    } else {
        fmt::layer()
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_filter(env_filter)
            .boxed()
    };

    let registry = tracing_subscriber::registry().with(fmt_layer);

    if let Some(endpoint) = config.observability.otlp_endpoint.as_ref() {
        let resource = opentelemetry_sdk::Resource::new(vec![KeyValue::new(
            "service.name",
            config.service.name.clone(),
        )]);

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_trace_config(sdktrace::Config::default().with_resource(resource))
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .install_batch(Tokio)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        registry.with(otel_layer).init();
    } else {
        registry.init();
    }

    let metrics_task = if config.observability.metrics_enabled {
        let handle = install_metrics_recorder()?;
        Some(spawn_metrics_server(
            config.observability.metrics_bind.clone(),
            handle,
        ))
    } else {
        None
    };

    info!("Observability initialized");

    Ok(ObservabilityGuard {
        _metrics_task: metrics_task,
    })
}

fn install_metrics_recorder() -> anyhow::Result<PrometheusHandle> {
    let builder = PrometheusBuilder::new();
    let handle = builder.install_recorder()?;
    Ok(handle)
}

fn spawn_metrics_server(bind: String, handle: PrometheusHandle) -> JoinHandle<()> {
    tokio::spawn(async move {
        match TcpListener::bind(&bind).await {
            Ok(listener) => {
                info!("Metrics server listening on {}", bind);
                loop {
                    match listener.accept().await {
                        Ok((mut socket, _)) => {
                            let handle = handle.clone();
                            tokio::spawn(async move {
                                if let Err(err) = serve_metrics(&mut socket, handle).await {
                                    warn!("Metrics request failed: {}", err);
                                }
                            });
                        }
                        Err(err) => {
                            warn!("Metrics accept error: {}", err);
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                    }
                }
            }
            Err(err) => {
                warn!("Failed to bind metrics server {}: {}", bind, err);
            }
        }
    })
}

async fn serve_metrics(
    socket: &mut tokio::net::TcpStream,
    handle: PrometheusHandle,
) -> anyhow::Result<()> {
    let mut buffer = [0u8; 1024];
    let n = socket.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..n]);

    let response = if request.starts_with("GET /metrics") {
        let body = handle.render();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
    } else {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string()
    };

    socket.write_all(response.as_bytes()).await?;
    Ok(())
}
