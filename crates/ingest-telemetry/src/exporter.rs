use crate::{Result, TelemetryConfig};

/// Exporter configuration
#[derive(Debug, Clone)]
pub struct ExporterConfig {
    pub jaeger_enabled: bool,
    pub otlp_enabled: bool,
    pub prometheus_enabled: bool,
}

impl From<&TelemetryConfig> for ExporterConfig {
    fn from(config: &TelemetryConfig) -> Self {
        Self {
            jaeger_enabled: config.jaeger_endpoint.is_some(),
            otlp_enabled: config.otlp_endpoint.is_some(),
            prometheus_enabled: config.prometheus_port.is_some(),
        }
    }
}

/// Manages telemetry exporters
pub struct ExporterManager {
    config: ExporterConfig,
    initialized: bool,
}

impl ExporterManager {
    pub fn new(config: &TelemetryConfig) -> Self {
        Self {
            config: ExporterConfig::from(config),
            initialized: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Use config to initialize appropriate exporters
        if self.config.jaeger_enabled {
            // TODO: Initialize Jaeger exporter
            tracing::info!("Jaeger exporter would be initialized");
        }

        if self.config.otlp_enabled {
            // TODO: Initialize OTLP exporter
            tracing::info!("OTLP exporter would be initialized");
        }

        if self.config.prometheus_enabled {
            // TODO: Initialize Prometheus exporter
            tracing::info!("Prometheus exporter would be initialized");
        }

        self.initialized = true;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Use config to shutdown appropriate exporters
        if self.config.jaeger_enabled {
            // TODO: Shutdown Jaeger exporter
            tracing::info!("Jaeger exporter would be shut down");
        }

        if self.config.otlp_enabled {
            // TODO: Shutdown OTLP exporter
            tracing::info!("OTLP exporter would be shut down");
        }

        if self.config.prometheus_enabled {
            // TODO: Shutdown Prometheus exporter
            tracing::info!("Prometheus exporter would be shut down");
        }

        self.initialized = false;
        Ok(())
    }

    pub fn config(&self) -> &ExporterConfig {
        &self.config
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_config_from_telemetry_config() {
        let telemetry_config = TelemetryConfig::default()
            .with_jaeger("http://localhost:14268")
            .with_prometheus(9090);

        let exporter_config = ExporterConfig::from(&telemetry_config);
        assert!(exporter_config.jaeger_enabled);
        assert!(!exporter_config.otlp_enabled);
        assert!(exporter_config.prometheus_enabled);
    }

    #[tokio::test]
    async fn test_exporter_manager_lifecycle() {
        let config = TelemetryConfig::default()
            .with_jaeger("http://localhost:14268")
            .with_prometheus(9090);
        let mut manager = ExporterManager::new(&config);

        assert!(!manager.is_initialized());
        assert!(manager.config().jaeger_enabled);
        assert!(manager.config().prometheus_enabled);
        assert!(!manager.config().otlp_enabled);

        manager.initialize().await.unwrap();
        assert!(manager.is_initialized());

        manager.shutdown().await.unwrap();
        assert!(!manager.is_initialized());
    }
}
