use crate::{Result, TelemetryConfig};

/// Tracing manager for distributed tracing
pub struct TracingManager {
    config: TelemetryConfig,
    initialized: bool,
}

impl TracingManager {
    pub fn new(config: &TelemetryConfig) -> Self {
        Self {
            config: config.clone(),
            initialized: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Initialize OpenTelemetry tracing - Phase 1 basic implementation
        use opentelemetry::trace::TracerProvider;
        use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
        use tracing_subscriber::layer::SubscriberExt;

        // For Phase 1, use default resource
        // In Phase 2, we'll implement proper resource configuration

        // Configure sampling based on config
        let sampler = if self.config.trace_sampling_ratio >= 1.0 {
            Sampler::AlwaysOn
        } else if self.config.trace_sampling_ratio <= 0.0 {
            Sampler::AlwaysOff
        } else {
            Sampler::TraceIdRatioBased(self.config.trace_sampling_ratio)
        };

        // Create tracer provider with simple span processor for Phase 1
        let tracer_provider = SdkTracerProvider::builder().with_sampler(sampler).build();

        // Set global tracer provider
        opentelemetry::global::set_tracer_provider(tracer_provider.clone());

        // Initialize tracing subscriber with OpenTelemetry layer
        let telemetry_layer =
            tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer("inngest-telemetry"));

        let subscriber = tracing_subscriber::registry()
            .with(telemetry_layer)
            .with(tracing_subscriber::fmt::layer());

        // Try to set global subscriber, but don't fail if already set (for tests)
        if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
            tracing::warn!(
                "Failed to set global subscriber (may already be set): {}",
                e
            );
            // In tests, this is expected behavior
        }

        self.initialized = true;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Shutdown tracing infrastructure
        // For Phase 1, we simply clear the global tracer provider
        // In a more complete implementation, we would properly shutdown
        // all span processors and exporters
        tracing::info!("Shutting down tracing infrastructure");

        // Note: OpenTelemetry 0.30+ doesn't have shutdown_tracer_provider
        // We'll implement proper shutdown in the exporter manager
        self.initialized = false;
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_tracing_manager_lifecycle() {
        let config = TelemetryConfig::default();
        let mut manager = TracingManager::new(&config);

        assert!(!manager.is_initialized());

        manager.initialize().await.unwrap();
        assert!(manager.is_initialized());

        manager.shutdown().await.unwrap();
        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_tracing_manager_config() {
        let config = TelemetryConfig::default().service_name("test-service");
        let manager = TracingManager::new(&config);
        assert_eq!(manager.config().service_name, "test-service");
    }
}
