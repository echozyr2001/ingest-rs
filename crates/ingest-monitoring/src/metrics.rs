use crate::error::{MonitoringError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Monitoring metrics collector
pub struct MonitoringMetrics {
    counters: Arc<RwLock<HashMap<String, u64>>>,
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<f64>>>>,
    initialized: Arc<RwLock<bool>>,
}

impl MonitoringMetrics {
    /// Create a new monitoring metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize the metrics system
    pub async fn initialize(&self) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        if *initialized {
            return Ok(());
        }

        // Initialize default metrics
        self.increment_counter("monitoring.system.starts", 1)
            .await?;
        self.set_gauge("monitoring.system.status", 1.0).await?;

        *initialized = true;
        println!("Monitoring metrics initialized");
        Ok(())
    }

    /// Shutdown the metrics system
    pub async fn shutdown(&self) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        if !*initialized {
            return Ok(());
        }

        self.increment_counter("monitoring.system.stops", 1).await?;
        self.set_gauge("monitoring.system.status", 0.0).await?;

        *initialized = false;
        println!("Monitoring metrics shut down");
        Ok(())
    }

    /// Increment a counter
    pub async fn increment_counter(&self, name: &str, value: u64) -> Result<()> {
        let mut counters = self.counters.write().await;
        let current = counters.get(name).copied().unwrap_or(0);
        counters.insert(name.to_string(), current + value);
        Ok(())
    }

    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) -> Result<()> {
        let mut gauges = self.gauges.write().await;
        gauges.insert(name.to_string(), value);
        Ok(())
    }

    /// Record a histogram value
    pub async fn record_histogram(&self, name: &str, value: f64) -> Result<()> {
        let mut histograms = self.histograms.write().await;
        let values = histograms.entry(name.to_string()).or_insert_with(Vec::new);
        values.push(value);

        // Keep only the last 1000 values to prevent memory growth
        if values.len() > 1000 {
            values.remove(0);
        }

        Ok(())
    }

    /// Record a duration as a histogram
    pub async fn record_duration(&self, name: &str, duration: Duration) -> Result<()> {
        let milliseconds = duration.as_millis() as f64;
        self.record_histogram(name, milliseconds).await
    }

    /// Get a counter value
    pub async fn get_counter(&self, name: &str) -> Option<u64> {
        let counters = self.counters.read().await;
        counters.get(name).copied()
    }

    /// Get a gauge value
    pub async fn get_gauge(&self, name: &str) -> Option<f64> {
        let gauges = self.gauges.read().await;
        gauges.get(name).copied()
    }

    /// Get histogram statistics
    pub async fn get_histogram_stats(&self, name: &str) -> Option<HistogramStats> {
        let histograms = self.histograms.read().await;
        if let Some(values) = histograms.get(name) {
            if values.is_empty() {
                return None;
            }

            let mut sorted_values = values.clone();
            sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let count = sorted_values.len();
            let sum: f64 = sorted_values.iter().sum();
            let mean = sum / count as f64;

            let min = sorted_values[0];
            let max = sorted_values[count - 1];

            let p50_index = (count as f64 * 0.5) as usize;
            let p95_index = (count as f64 * 0.95) as usize;
            let p99_index = (count as f64 * 0.99) as usize;

            let p50 = sorted_values[p50_index.min(count - 1)];
            let p95 = sorted_values[p95_index.min(count - 1)];
            let p99 = sorted_values[p99_index.min(count - 1)];

            Some(HistogramStats {
                count,
                sum,
                mean,
                min,
                max,
                p50,
                p95,
                p99,
            })
        } else {
            None
        }
    }

    /// Get all metrics as a snapshot
    pub async fn snapshot(&self) -> Result<String> {
        let counters = self.counters.read().await;
        let gauges = self.gauges.read().await;
        let histograms = self.histograms.read().await;

        let mut snapshot = MetricsSnapshot {
            counters: counters.clone(),
            gauges: gauges.clone(),
            histograms: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        // Calculate histogram stats
        for (name, values) in histograms.iter() {
            if !values.is_empty() {
                let mut sorted_values = values.clone();
                sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let count = sorted_values.len();
                let sum: f64 = sorted_values.iter().sum();
                let mean = sum / count as f64;

                let min = sorted_values[0];
                let max = sorted_values[count - 1];

                let p50_index = (count as f64 * 0.5) as usize;
                let p95_index = (count as f64 * 0.95) as usize;
                let p99_index = (count as f64 * 0.99) as usize;

                let p50 = sorted_values[p50_index.min(count - 1)];
                let p95 = sorted_values[p95_index.min(count - 1)];
                let p99 = sorted_values[p99_index.min(count - 1)];

                snapshot.histograms.insert(
                    name.clone(),
                    HistogramStats {
                        count,
                        sum,
                        mean,
                        min,
                        max,
                        p50,
                        p95,
                        p99,
                    },
                );
            }
        }

        serde_json::to_string_pretty(&snapshot)
            .map_err(|e| MonitoringError::metrics(format!("Failed to serialize metrics: {e}")))
    }

    /// Check if the metrics system is initialized
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Clear all metrics
    pub async fn clear(&self) -> Result<()> {
        let mut counters = self.counters.write().await;
        let mut gauges = self.gauges.write().await;
        let mut histograms = self.histograms.write().await;

        counters.clear();
        gauges.clear();
        histograms.clear();

        Ok(())
    }
}

impl Default for MonitoringMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Histogram statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramStats {
    /// Number of samples
    pub count: usize,
    /// Sum of all values
    pub sum: f64,
    /// Mean value
    pub mean: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// 50th percentile
    pub p50: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
}

/// Metrics snapshot
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Counter values
    pub counters: HashMap<String, u64>,
    /// Gauge values
    pub gauges: HashMap<String, f64>,
    /// Histogram statistics
    pub histograms: HashMap<String, HistogramStats>,
    /// Timestamp of the snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_monitoring_metrics_creation() {
        let metrics = MonitoringMetrics::new();
        assert!(!metrics.is_initialized().await);
    }

    #[tokio::test]
    async fn test_counter_operations() {
        let metrics = MonitoringMetrics::new();

        // Test incrementing a counter
        metrics.increment_counter("test.counter", 5).await.unwrap();
        let value = metrics.get_counter("test.counter").await;
        assert_eq!(value, Some(5));

        // Test incrementing again
        metrics.increment_counter("test.counter", 3).await.unwrap();
        let value = metrics.get_counter("test.counter").await;
        assert_eq!(value, Some(8));
    }

    #[tokio::test]
    async fn test_gauge_operations() {
        let metrics = MonitoringMetrics::new();

        // Test setting a gauge
        metrics.set_gauge("test.gauge", 42.5).await.unwrap();
        let value = metrics.get_gauge("test.gauge").await;
        assert_eq!(value, Some(42.5));

        // Test updating the gauge
        metrics.set_gauge("test.gauge", 100.0).await.unwrap();
        let value = metrics.get_gauge("test.gauge").await;
        assert_eq!(value, Some(100.0));
    }

    #[tokio::test]
    async fn test_histogram_operations() {
        let metrics = MonitoringMetrics::new();

        // Test recording histogram values
        metrics
            .record_histogram("test.histogram", 10.0)
            .await
            .unwrap();
        metrics
            .record_histogram("test.histogram", 20.0)
            .await
            .unwrap();
        metrics
            .record_histogram("test.histogram", 30.0)
            .await
            .unwrap();

        let stats = metrics.get_histogram_stats("test.histogram").await;
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.sum, 60.0);
        assert_eq!(stats.mean, 20.0);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 30.0);
    }

    #[tokio::test]
    async fn test_duration_recording() {
        let metrics = MonitoringMetrics::new();

        let duration = Duration::from_millis(500);
        metrics
            .record_duration("test.duration", duration)
            .await
            .unwrap();

        let stats = metrics.get_histogram_stats("test.duration").await;
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.count, 1);
        assert_eq!(stats.sum, 500.0);
    }

    #[tokio::test]
    async fn test_metrics_snapshot() {
        let metrics = MonitoringMetrics::new();

        metrics.increment_counter("test.counter", 10).await.unwrap();
        metrics.set_gauge("test.gauge", 25.5).await.unwrap();
        metrics
            .record_histogram("test.histogram", 15.0)
            .await
            .unwrap();

        let snapshot = metrics.snapshot().await.unwrap();
        assert!(snapshot.contains("test.counter"));
        assert!(snapshot.contains("test.gauge"));
        assert!(snapshot.contains("test.histogram"));
    }

    #[tokio::test]
    async fn test_metrics_clear() {
        let metrics = MonitoringMetrics::new();

        metrics.increment_counter("test.counter", 10).await.unwrap();
        metrics.set_gauge("test.gauge", 25.5).await.unwrap();

        assert_eq!(metrics.get_counter("test.counter").await, Some(10));
        assert_eq!(metrics.get_gauge("test.gauge").await, Some(25.5));

        metrics.clear().await.unwrap();

        assert_eq!(metrics.get_counter("test.counter").await, None);
        assert_eq!(metrics.get_gauge("test.gauge").await, None);
    }
}
