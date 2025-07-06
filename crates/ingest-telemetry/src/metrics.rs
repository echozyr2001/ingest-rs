use crate::{Result, TelemetryConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Counter metric
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    description: String,
    value: Arc<RwLock<f64>>,
}

impl Counter {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            value: Arc::new(RwLock::new(0.0)),
        }
    }

    pub async fn increment(&self) {
        let mut value = self.value.write().await;
        *value += 1.0;
    }

    pub async fn add(&self, delta: f64) {
        let mut value = self.value.write().await;
        *value += delta;
    }

    pub async fn get(&self) -> f64 {
        *self.value.read().await
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Gauge metric
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    description: String,
    value: Arc<RwLock<f64>>,
}

impl Gauge {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            value: Arc::new(RwLock::new(0.0)),
        }
    }

    pub async fn set(&self, value: f64) {
        let mut current = self.value.write().await;
        *current = value;
    }

    pub async fn get(&self) -> f64 {
        *self.value.read().await
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Histogram metric
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    description: String,
    values: Arc<RwLock<Vec<f64>>>,
}

impl Histogram {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            values: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn observe(&self, value: f64) {
        let mut values = self.values.write().await;
        values.push(value);
    }

    pub async fn count(&self) -> usize {
        self.values.read().await.len()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Metrics collector
pub struct MetricsCollector {
    config: TelemetryConfig,
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    initialized: bool,
}

impl MetricsCollector {
    pub fn new(config: &TelemetryConfig) -> Self {
        Self {
            config: config.clone(),
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            initialized: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Initialize Prometheus registry with config
        if self.config.prometheus_port.is_some() {
            use prometheus::Registry;

            // Create a new Prometheus registry
            let _registry = Registry::new();

            // For Phase 1, we just create the basic registry
            // In Phase 2, we would:
            // - Register default collectors (process, runtime metrics)
            // - Start HTTP server for metrics endpoint
            // - Integrate with OpenTelemetry metrics pipeline

            tracing::info!(
                "Prometheus registry initialized for port: {:?}",
                self.config.prometheus_port
            );
        }

        self.initialized = true;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }

        // Clear all metrics collections
        self.counters.write().await.clear();
        self.gauges.write().await.clear();
        self.histograms.write().await.clear();

        self.initialized = false;
        Ok(())
    }

    pub async fn counter(&self, name: &str, description: &str) -> Result<Counter> {
        let counter = Counter::new(name, description);

        // Store in collection for management
        self.counters
            .write()
            .await
            .insert(name.to_string(), counter.clone());

        Ok(counter)
    }

    pub async fn gauge(&self, name: &str, description: &str) -> Result<Gauge> {
        let gauge = Gauge::new(name, description);

        // Store in collection for management
        self.gauges
            .write()
            .await
            .insert(name.to_string(), gauge.clone());

        Ok(gauge)
    }

    pub async fn histogram(&self, name: &str, description: &str) -> Result<Histogram> {
        let histogram = Histogram::new(name, description);

        // Store in collection for management
        self.histograms
            .write()
            .await
            .insert(name.to_string(), histogram.clone());

        Ok(histogram)
    }

    pub fn record(&self, name: &str, value: f64, labels: &[(&str, &str)]) -> Result<()> {
        // Record metric with labels - Phase 1 basic implementation
        tracing::debug!(
            "Recording metric: {} = {} with labels: {:?}",
            name,
            value,
            labels
        );

        // For Phase 1, we just log the metric recording
        // In Phase 2, we would:
        // 1. Look up or create the metric in Prometheus registry
        // 2. Apply labels to the metric
        // 3. Record the value

        // Basic validation
        if name.is_empty() {
            return Err(crate::TelemetryError::metrics(
                "Metric name cannot be empty",
            ));
        }

        if !value.is_finite() {
            return Err(crate::TelemetryError::metrics(
                "Metric value must be finite",
            ));
        }

        // TODO: Implement actual metric recording to Prometheus registry
        tracing::trace!("Metric recorded successfully: {}", name);

        Ok(())
    }

    pub async fn snapshot(&self) -> Result<String> {
        let mut output = String::new();
        output.push_str("# Metrics snapshot\n");

        // Include counters
        let counters = self.counters.read().await;
        for (name, counter) in counters.iter() {
            output.push_str(&format!(
                "# HELP {} {}\n# TYPE {} counter\n{} {}\n",
                name,
                counter.description(),
                name,
                name,
                counter.get().await
            ));
        }

        // Include gauges
        let gauges = self.gauges.read().await;
        for (name, gauge) in gauges.iter() {
            output.push_str(&format!(
                "# HELP {} {}\n# TYPE {} gauge\n{} {}\n",
                name,
                gauge.description(),
                name,
                name,
                gauge.get().await
            ));
        }

        // Include histograms
        let histograms = self.histograms.read().await;
        for (name, histogram) in histograms.iter() {
            output.push_str(&format!(
                "# HELP {} {}\n# TYPE {} histogram\n{}_count {}\n",
                name,
                histogram.description(),
                name,
                name,
                histogram.count().await
            ));
        }

        Ok(output)
    }

    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }

    pub async fn list_counters(&self) -> Vec<String> {
        self.counters.read().await.keys().cloned().collect()
    }

    pub async fn list_gauges(&self) -> Vec<String> {
        self.gauges.read().await.keys().cloned().collect()
    }

    pub async fn list_histograms(&self) -> Vec<String> {
        self.histograms.read().await.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_counter() {
        let counter = Counter::new("test_counter", "Test counter");
        assert_eq!(counter.get().await, 0.0);
        assert_eq!(counter.name(), "test_counter");
        assert_eq!(counter.description(), "Test counter");

        counter.increment().await;
        assert_eq!(counter.get().await, 1.0);

        counter.add(5.0).await;
        assert_eq!(counter.get().await, 6.0);
    }

    #[tokio::test]
    async fn test_gauge() {
        let gauge = Gauge::new("test_gauge", "Test gauge");
        assert_eq!(gauge.get().await, 0.0);
        assert_eq!(gauge.name(), "test_gauge");
        assert_eq!(gauge.description(), "Test gauge");

        gauge.set(42.0).await;
        assert_eq!(gauge.get().await, 42.0);
    }

    #[tokio::test]
    async fn test_histogram() {
        let histogram = Histogram::new("test_histogram", "Test histogram");
        assert_eq!(histogram.count().await, 0);
        assert_eq!(histogram.name(), "test_histogram");
        assert_eq!(histogram.description(), "Test histogram");

        histogram.observe(1.0).await;
        histogram.observe(2.0).await;
        assert_eq!(histogram.count().await, 2);
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let config = TelemetryConfig::default().with_prometheus(9090);
        let mut collector = MetricsCollector::new(&config);

        collector.initialize().await.unwrap();
        assert!(collector.initialized);
        assert_eq!(collector.config().prometheus_port, Some(9090));

        let counter = collector
            .counter("test_counter", "Test description")
            .await
            .unwrap();
        counter.increment().await;

        let gauge = collector.gauge("test_gauge", "Test gauge").await.unwrap();
        gauge.set(42.0).await;

        let histogram = collector
            .histogram("test_histogram", "Test histogram")
            .await
            .unwrap();
        histogram.observe(1.5).await;

        // Test collections are used
        assert_eq!(collector.list_counters().await.len(), 1);
        assert_eq!(collector.list_gauges().await.len(), 1);
        assert_eq!(collector.list_histograms().await.len(), 1);

        let snapshot = collector.snapshot().await.unwrap();
        assert!(snapshot.contains("test_counter"));
        assert!(snapshot.contains("test_gauge"));
        assert!(snapshot.contains("test_histogram"));

        collector.shutdown().await.unwrap();
        assert!(!collector.initialized);
        assert_eq!(collector.list_counters().await.len(), 0);
    }
}
