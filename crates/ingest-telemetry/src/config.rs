use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Telemetry configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct TelemetryConfig {
    /// Service name for telemetry
    pub service_name: String,
    /// Service version
    pub service_version: Option<String>,
    /// Service environment (dev, staging, prod)
    pub environment: Option<String>,
    /// Enable distributed tracing
    pub tracing_enabled: bool,
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Enable structured logging
    pub logging_enabled: bool,
    /// Jaeger endpoint for trace export
    pub jaeger_endpoint: Option<String>,
    /// OTLP endpoint for trace/metrics export
    pub otlp_endpoint: Option<String>,
    /// Prometheus metrics port
    pub prometheus_port: Option<u16>,
    /// Sampling ratio for traces (0.0 to 1.0)
    pub trace_sampling_ratio: f64,
    /// Batch timeout for trace export
    pub trace_batch_timeout: Duration,
    /// Maximum batch size for traces
    pub trace_batch_size: usize,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Custom resource attributes
    pub resource_attributes: std::collections::HashMap<String, String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        let mut resource_attributes = std::collections::HashMap::new();
        resource_attributes.insert("service.name".to_string(), "inngest".to_string());
        resource_attributes.insert(
            "service.version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        Self {
            service_name: "inngest".to_string(),
            service_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            environment: Some("development".to_string()),
            tracing_enabled: true,
            metrics_enabled: true,
            logging_enabled: true,
            jaeger_endpoint: None,
            otlp_endpoint: None,
            prometheus_port: Some(9090),
            trace_sampling_ratio: 1.0,
            trace_batch_timeout: Duration::from_secs(5),
            trace_batch_size: 512,
            metrics_interval: Duration::from_secs(10),
            resource_attributes,
        }
    }
}

impl TelemetryConfig {
    /// Create a new telemetry configuration with service name
    pub fn new(service_name: impl Into<String>) -> Self {
        Self::default().service_name(service_name)
    }

    /// Validate the configuration
    pub fn validate(&self) -> crate::Result<()> {
        if self.service_name.is_empty() {
            return Err(crate::TelemetryError::configuration(
                "Service name cannot be empty",
            ));
        }

        if self.trace_sampling_ratio < 0.0 || self.trace_sampling_ratio > 1.0 {
            return Err(crate::TelemetryError::configuration(
                "Trace sampling ratio must be between 0.0 and 1.0",
            ));
        }

        if self.trace_batch_size == 0 {
            return Err(crate::TelemetryError::configuration(
                "Trace batch size must be > 0",
            ));
        }

        if let Some(ref endpoint) = self.jaeger_endpoint {
            url::Url::parse(endpoint).map_err(|e| {
                crate::TelemetryError::configuration(format!("Invalid Jaeger endpoint: {e}"))
            })?;
        }

        if let Some(ref endpoint) = self.otlp_endpoint {
            url::Url::parse(endpoint).map_err(|e| {
                crate::TelemetryError::configuration(format!("Invalid OTLP endpoint: {e}"))
            })?;
        }

        Ok(())
    }

    /// Enable Jaeger tracing with endpoint
    pub fn with_jaeger(mut self, endpoint: impl Into<String>) -> Self {
        self.jaeger_endpoint = Some(endpoint.into());
        self.tracing_enabled = true;
        self
    }

    /// Enable OTLP export with endpoint
    pub fn with_otlp(mut self, endpoint: impl Into<String>) -> Self {
        self.otlp_endpoint = Some(endpoint.into());
        self.tracing_enabled = true;
        self.metrics_enabled = true;
        self
    }

    /// Enable Prometheus metrics on specific port
    pub fn with_prometheus(mut self, port: u16) -> Self {
        self.prometheus_port = Some(port);
        self.metrics_enabled = true;
        self
    }

    /// Set trace sampling ratio
    pub fn with_sampling(mut self, ratio: f64) -> Self {
        self.trace_sampling_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Add custom resource attribute
    pub fn with_resource_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.resource_attributes.insert(key.into(), value.into());
        self
    }

    /// Create configuration for development environment
    pub fn development() -> Self {
        Self::default()
            .environment("development")
            .trace_sampling_ratio(1.0)
            .with_prometheus(9090)
    }

    /// Create configuration for production environment
    pub fn production() -> Self {
        Self::default()
            .environment("production")
            .trace_sampling_ratio(0.1)
            .with_prometheus(9090)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "inngest");
        assert!(config.tracing_enabled);
        assert!(config.metrics_enabled);
        assert_eq!(config.trace_sampling_ratio, 1.0);
    }

    #[test]
    fn test_telemetry_config_validation_success() {
        let config = TelemetryConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_telemetry_config_validation_empty_service_name() {
        let config = TelemetryConfig::default().service_name("");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_telemetry_config_validation_invalid_sampling_ratio() {
        let config = TelemetryConfig::default().trace_sampling_ratio(1.5);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_telemetry_config_with_jaeger() {
        let config = TelemetryConfig::default().with_jaeger("http://localhost:14268/api/traces");
        assert_eq!(
            config.jaeger_endpoint,
            Some("http://localhost:14268/api/traces".to_string())
        );
        assert!(config.tracing_enabled);
    }

    #[test]
    fn test_telemetry_config_with_otlp() {
        let config = TelemetryConfig::default().with_otlp("http://localhost:4317");
        assert_eq!(
            config.otlp_endpoint,
            Some("http://localhost:4317".to_string())
        );
        assert!(config.tracing_enabled);
        assert!(config.metrics_enabled);
    }

    #[test]
    fn test_telemetry_config_development() {
        let config = TelemetryConfig::development();
        assert_eq!(config.environment, Some("development".to_string()));
        assert_eq!(config.trace_sampling_ratio, 1.0);
        assert_eq!(config.prometheus_port, Some(9090));
    }

    #[test]
    fn test_telemetry_config_production() {
        let config = TelemetryConfig::production();
        assert_eq!(config.environment, Some("production".to_string()));
        assert_eq!(config.trace_sampling_ratio, 0.1);
    }

    #[test]
    fn test_telemetry_config_resource_attributes() {
        let config =
            TelemetryConfig::default().with_resource_attribute("custom.key", "custom.value");
        assert_eq!(
            config.resource_attributes.get("custom.key"),
            Some(&"custom.value".to_string())
        );
    }

    #[test]
    fn test_telemetry_config_serialization() {
        let config = TelemetryConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: TelemetryConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }
}
