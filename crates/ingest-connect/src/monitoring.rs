//! Live monitoring and dashboard system
//!
//! This module provides real-time monitoring capabilities for system health,
//! performance metrics, and execution analytics.

use crate::types::{ConnectionId, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, broadcast};

/// Live system metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveMetrics {
    /// Number of active function executions
    pub active_executions: u64,
    /// Executions per second (current rate)
    pub executions_per_second: f64,
    /// Average execution time in milliseconds
    pub average_execution_time_ms: f64,
    /// Current error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Queue depth (pending executions)
    pub queue_depth: u64,
    /// System load metrics
    pub system_load: SystemLoad,
    /// Memory usage metrics
    pub memory_usage: MemoryUsage,
    /// Network metrics
    pub network_metrics: NetworkMetrics,
    /// Timestamp of metrics collection
    pub timestamp: u64,
}

/// System load information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLoad {
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_usage: f64,
    /// Memory usage percentage (0.0 to 100.0)
    pub memory_usage: f64,
    /// Disk I/O operations per second
    pub disk_iops: f64,
    /// Network I/O bytes per second
    pub network_io_bps: f64,
}

/// Memory usage details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    /// Total memory in bytes
    pub total_bytes: u64,
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Available memory in bytes
    pub available_bytes: u64,
    /// Memory usage percentage
    pub usage_percentage: f64,
}

/// Network metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Bytes received per second
    pub bytes_received_per_sec: f64,
    /// Bytes sent per second
    pub bytes_sent_per_sec: f64,
    /// Active connections count
    pub active_connections: u64,
    /// Connection errors per second
    pub connection_errors_per_sec: f64,
}

/// Performance analytics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalytics {
    /// Function execution statistics
    pub function_stats: Vec<FunctionStats>,
    /// Execution time distribution
    pub execution_time_distribution: TimeDistribution,
    /// Error analysis
    pub error_analysis: ErrorAnalysis,
    /// Throughput trends
    pub throughput_trends: ThroughputTrends,
    /// Timestamp of analytics collection
    pub timestamp: u64,
}

/// Function-specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionStats {
    /// Function ID
    pub function_id: String,
    /// Function name
    pub function_name: String,
    /// Total executions
    pub total_executions: u64,
    /// Successful executions
    pub successful_executions: u64,
    /// Failed executions
    pub failed_executions: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// P95 execution time in milliseconds
    pub p95_execution_time_ms: f64,
    /// P99 execution time in milliseconds
    pub p99_execution_time_ms: f64,
    /// Executions per minute
    pub executions_per_minute: f64,
}

/// Execution time distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDistribution {
    /// Histogram buckets (time ranges in milliseconds)
    pub buckets: Vec<TimeBucket>,
    /// Percentile values
    pub percentiles: HashMap<String, f64>,
}

/// Time bucket for histogram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBucket {
    /// Lower bound in milliseconds
    pub lower_bound_ms: f64,
    /// Upper bound in milliseconds
    pub upper_bound_ms: f64,
    /// Count of executions in this bucket
    pub count: u64,
}

/// Error analysis data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorAnalysis {
    /// Total error count
    pub total_errors: u64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Error types and counts
    pub error_types: HashMap<String, u64>,
    /// Recent error trend
    pub error_trend: Vec<ErrorDataPoint>,
}

/// Error data point for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDataPoint {
    /// Timestamp
    pub timestamp: u64,
    /// Error count in this time window
    pub error_count: u64,
    /// Total executions in this time window
    pub total_executions: u64,
}

/// Throughput trends data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputTrends {
    /// Throughput data points over time
    pub data_points: Vec<ThroughputDataPoint>,
    /// Current throughput (executions per second)
    pub current_throughput: f64,
    /// Peak throughput in the last hour
    pub peak_throughput: f64,
    /// Average throughput in the last hour
    pub average_throughput: f64,
}

/// Throughput data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputDataPoint {
    /// Timestamp
    pub timestamp: u64,
    /// Executions per second
    pub executions_per_second: f64,
    /// Events per second
    pub events_per_second: f64,
}

/// Dashboard update message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DashboardUpdate {
    /// Live metrics update
    LiveMetrics(LiveMetrics),
    /// Performance analytics update
    PerformanceAnalytics(PerformanceAnalytics),
    /// Alert notification
    Alert {
        level: AlertLevel,
        message: String,
        details: serde_json::Value,
        timestamp: u64,
    },
    /// System status change
    SystemStatus {
        status: SystemStatus,
        message: String,
        timestamp: u64,
    },
}

/// Alert levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// System status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Maintenance,
}

fn default_true() -> bool {
    true
}

fn default_update_interval() -> u64 {
    1000
}

/// Dashboard subscription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSubscription {
    /// Include live metrics
    #[serde(default = "default_true")]
    pub include_metrics: bool,
    /// Include performance analytics
    #[serde(default = "default_true")]
    pub include_analytics: bool,
    /// Include alerts
    #[serde(default = "default_true")]
    pub include_alerts: bool,
    /// Include system status
    #[serde(default = "default_true")]
    pub include_status: bool,
    /// Update interval in milliseconds
    #[serde(default = "default_update_interval")]
    pub update_interval_ms: u64,
    /// Alert level filter
    pub alert_level_filter: Option<Vec<AlertLevel>>,
}

impl Default for DashboardSubscription {
    fn default() -> Self {
        Self {
            include_metrics: true,
            include_analytics: true,
            include_alerts: true,
            include_status: true,
            update_interval_ms: 1000,
            alert_level_filter: None,
        }
    }
}

/// Dashboard connection state
#[derive(Debug, Clone)]
pub struct DashboardConnection {
    pub connection_id: ConnectionId,
    pub session_id: SessionId,
    pub subscription: DashboardSubscription,
    pub created_at: Instant,
    pub last_update: Instant,
    pub updates_sent: u64,
}

impl DashboardConnection {
    /// Create a new dashboard connection
    pub fn new(
        connection_id: ConnectionId,
        session_id: SessionId,
        subscription: DashboardSubscription,
    ) -> Self {
        let now = Instant::now();
        Self {
            connection_id,
            session_id,
            subscription,
            created_at: now,
            last_update: now,
            updates_sent: 0,
        }
    }

    /// Check if connection should receive an update
    pub fn should_update(&self) -> bool {
        let interval = Duration::from_millis(self.subscription.update_interval_ms);
        self.last_update.elapsed() >= interval
    }

    /// Mark update as sent
    pub fn mark_updated(&mut self) {
        self.last_update = Instant::now();
        self.updates_sent += 1;
    }

    /// Check if connection matches alert filter
    pub fn matches_alert(&self, level: &AlertLevel) -> bool {
        self.subscription
            .alert_level_filter
            .as_ref()
            .is_none_or(|filters| filters.contains(level))
    }
}

/// Live monitoring system
pub struct LiveMonitor {
    /// Active dashboard connections
    connections: Arc<RwLock<HashMap<ConnectionId, DashboardConnection>>>,
    /// Broadcast channel for dashboard updates
    sender: broadcast::Sender<DashboardUpdate>,
    /// Metrics collection interval
    collection_interval: Duration,
    /// Analytics calculation interval
    analytics_interval: Duration,
}

impl LiveMonitor {
    /// Create a new live monitor
    pub fn new(
        buffer_size: usize,
        collection_interval: Duration,
        analytics_interval: Duration,
    ) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            sender,
            collection_interval,
            analytics_interval,
        }
    }

    /// Subscribe to dashboard updates
    pub async fn subscribe_dashboard(
        &self,
        connection_id: ConnectionId,
        session_id: SessionId,
        subscription: DashboardSubscription,
    ) -> broadcast::Receiver<DashboardUpdate> {
        let connection = DashboardConnection::new(connection_id, session_id, subscription);

        let mut connections = self.connections.write().await;
        connections.insert(connection_id, connection);

        self.sender.subscribe()
    }

    /// Unsubscribe from dashboard updates
    pub async fn unsubscribe_dashboard(&self, connection_id: ConnectionId) {
        let mut connections = self.connections.write().await;
        connections.remove(&connection_id);
    }

    /// Broadcast live metrics
    pub async fn broadcast_metrics(&self, metrics: LiveMetrics) -> crate::Result<usize> {
        let update = DashboardUpdate::LiveMetrics(metrics);
        self.broadcast_update(update).await
    }

    /// Broadcast performance analytics
    pub async fn broadcast_analytics(
        &self,
        analytics: PerformanceAnalytics,
    ) -> crate::Result<usize> {
        let update = DashboardUpdate::PerformanceAnalytics(analytics);
        self.broadcast_update(update).await
    }

    /// Broadcast alert
    pub async fn broadcast_alert(
        &self,
        level: AlertLevel,
        message: String,
        details: serde_json::Value,
    ) -> crate::Result<usize> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let update = DashboardUpdate::Alert {
            level,
            message,
            details,
            timestamp,
        };

        self.broadcast_update(update).await
    }

    /// Broadcast system status
    pub async fn broadcast_status(
        &self,
        status: SystemStatus,
        message: String,
    ) -> crate::Result<usize> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let update = DashboardUpdate::SystemStatus {
            status,
            message,
            timestamp,
        };

        self.broadcast_update(update).await
    }

    /// Get metrics collection interval
    pub fn collection_interval(&self) -> Duration {
        self.collection_interval
    }

    /// Get analytics calculation interval
    pub fn analytics_interval(&self) -> Duration {
        self.analytics_interval
    }

    /// Get current dashboard connection count
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// Broadcast update to matching connections
    async fn broadcast_update(&self, update: DashboardUpdate) -> crate::Result<usize> {
        let connections = self.connections.read().await;
        let matching_count = connections
            .values()
            .filter(|conn| self.connection_matches_update(conn, &update))
            .count();

        // Broadcast to all connections (filtering happens on receiver side)
        if self.sender.send(update).is_err() {
            // No active receivers, which is fine
        }

        Ok(matching_count)
    }

    /// Check if connection should receive update
    fn connection_matches_update(
        &self,
        conn: &DashboardConnection,
        update: &DashboardUpdate,
    ) -> bool {
        match update {
            DashboardUpdate::LiveMetrics(_) => conn.subscription.include_metrics,
            DashboardUpdate::PerformanceAnalytics(_) => conn.subscription.include_analytics,
            DashboardUpdate::Alert { level, .. } => {
                conn.subscription.include_alerts && conn.matches_alert(level)
            }
            DashboardUpdate::SystemStatus { .. } => conn.subscription.include_status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_dashboard_subscription_default() {
        let subscription = DashboardSubscription::default();

        assert!(subscription.include_metrics);
        assert!(subscription.include_analytics);
        assert!(subscription.include_alerts);
        assert!(subscription.include_status);
        assert_eq!(subscription.update_interval_ms, 1000);
    }

    #[test]
    fn test_dashboard_connection_creation() {
        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let subscription = DashboardSubscription::default();

        let connection = DashboardConnection::new(connection_id, session_id, subscription);

        assert_eq!(connection.connection_id, connection_id);
        assert_eq!(connection.session_id, session_id);
        assert_eq!(connection.updates_sent, 0);
    }

    #[test]
    fn test_dashboard_connection_alert_filter() {
        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let subscription = DashboardSubscription {
            alert_level_filter: Some(vec![AlertLevel::Error, AlertLevel::Critical]),
            ..Default::default()
        };

        let connection = DashboardConnection::new(connection_id, session_id, subscription);

        assert!(connection.matches_alert(&AlertLevel::Error));
        assert!(connection.matches_alert(&AlertLevel::Critical));
        assert!(!connection.matches_alert(&AlertLevel::Warning));
        assert!(!connection.matches_alert(&AlertLevel::Info));
    }

    #[tokio::test]
    async fn test_live_monitor_subscription() {
        let monitor = LiveMonitor::new(1000, Duration::from_secs(1), Duration::from_secs(60));

        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let subscription = DashboardSubscription::default();

        let _receiver = monitor
            .subscribe_dashboard(connection_id, session_id, subscription)
            .await;

        assert_eq!(monitor.connection_count().await, 1);

        monitor.unsubscribe_dashboard(connection_id).await;
        assert_eq!(monitor.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_live_monitor_metrics_broadcast() {
        let monitor = LiveMonitor::new(1000, Duration::from_secs(1), Duration::from_secs(60));

        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let subscription = DashboardSubscription::default();

        let mut receiver = monitor
            .subscribe_dashboard(connection_id, session_id, subscription)
            .await;

        let metrics = LiveMetrics {
            active_executions: 10,
            executions_per_second: 5.0,
            average_execution_time_ms: 100.0,
            error_rate: 0.01,
            queue_depth: 5,
            system_load: SystemLoad {
                cpu_usage: 50.0,
                memory_usage: 60.0,
                disk_iops: 100.0,
                network_io_bps: 1000.0,
            },
            memory_usage: MemoryUsage {
                total_bytes: 1000000,
                used_bytes: 600000,
                available_bytes: 400000,
                usage_percentage: 60.0,
            },
            network_metrics: NetworkMetrics {
                bytes_received_per_sec: 1000.0,
                bytes_sent_per_sec: 800.0,
                active_connections: 50,
                connection_errors_per_sec: 0.1,
            },
            timestamp: 1234567890,
        };

        let count = monitor.broadcast_metrics(metrics.clone()).await.unwrap();
        assert_eq!(count, 1);

        // Receive the update
        let received = receiver.recv().await.unwrap();
        match received {
            DashboardUpdate::LiveMetrics(received_metrics) => {
                assert_eq!(received_metrics.active_executions, 10);
                assert_eq!(received_metrics.executions_per_second, 5.0);
            }
            _ => panic!("Unexpected update type"),
        }
    }

    #[tokio::test]
    async fn test_live_monitor_configuration() {
        let collection_interval = Duration::from_secs(5);
        let analytics_interval = Duration::from_secs(30);
        let monitor = LiveMonitor::new(1000, collection_interval, analytics_interval);

        assert_eq!(monitor.collection_interval(), collection_interval);
        assert_eq!(monitor.analytics_interval(), analytics_interval);
    }
}
