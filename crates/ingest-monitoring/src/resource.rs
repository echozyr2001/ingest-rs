use crate::error::{MonitoringError, Result};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ResourceUsage {
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_percent: f64,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Memory usage percentage (0.0 to 100.0)
    pub memory_percent: f64,
    /// Disk usage in bytes
    pub disk_bytes: u64,
    /// Disk usage percentage (0.0 to 100.0)
    pub disk_percent: f64,
    /// Network bytes received
    pub network_rx_bytes: u64,
    /// Network bytes transmitted
    pub network_tx_bytes: u64,
    /// Number of open file descriptors
    pub open_fds: u32,
    /// Timestamp of the measurement
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_bytes: 0,
            memory_percent: 0.0,
            disk_bytes: 0,
            disk_percent: 0.0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            open_fds: 0,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Resource monitor for tracking system resource usage
pub struct ResourceMonitor {
    current_usage: Arc<RwLock<ResourceUsage>>,
    monitoring_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    running: Arc<RwLock<bool>>,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new() -> Self {
        Self {
            current_usage: Arc::new(RwLock::new(ResourceUsage::default())),
            monitoring_handle: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start resource monitoring
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(MonitoringError::resource(
                "Resource monitoring already running",
            ));
        }

        let current_usage = self.current_usage.clone();
        let running_flag = self.running.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                // Check if we should stop
                {
                    let running = running_flag.read().await;
                    if !*running {
                        break;
                    }
                }

                // Collect resource usage
                let usage = Self::collect_usage().await;

                // Update current usage
                {
                    let mut current = current_usage.write().await;
                    *current = usage;
                }
            }
        });

        let mut monitoring_handle = self.monitoring_handle.write().await;
        *monitoring_handle = Some(handle);
        *running = true;

        println!("Resource monitoring started");
        Ok(())
    }

    /// Stop resource monitoring
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Ok(());
        }

        *running = false;

        let mut monitoring_handle = self.monitoring_handle.write().await;
        if let Some(handle) = monitoring_handle.take() {
            handle.abort();
        }

        println!("Resource monitoring stopped");
        Ok(())
    }

    /// Get current resource usage
    pub async fn current_usage(&self) -> ResourceUsage {
        let usage = self.current_usage.read().await;
        usage.clone()
    }

    /// Check if monitoring is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Collect current resource usage
    async fn collect_usage() -> ResourceUsage {
        // Note: This is a basic implementation that returns mock data
        // In a real implementation, you would use system APIs or libraries
        // like sysinfo to get actual resource usage

        let timestamp = chrono::Utc::now();

        // Mock data for demonstration
        // In production, replace with actual system calls
        ResourceUsage {
            cpu_percent: Self::get_cpu_usage().await,
            memory_bytes: Self::get_memory_usage().await,
            memory_percent: Self::get_memory_percent().await,
            disk_bytes: Self::get_disk_usage().await,
            disk_percent: Self::get_disk_percent().await,
            network_rx_bytes: Self::get_network_rx().await,
            network_tx_bytes: Self::get_network_tx().await,
            open_fds: Self::get_open_fds().await,
            timestamp,
        }
    }

    /// Get CPU usage percentage
    async fn get_cpu_usage() -> f64 {
        // Mock implementation - in production, use system APIs
        // This could use /proc/stat on Linux or similar mechanisms
        let base = 5.0;
        let variation = (chrono::Utc::now().timestamp() % 20) as f64;
        (base + variation).min(100.0)
    }

    /// Get memory usage in bytes
    async fn get_memory_usage() -> u64 {
        // Mock implementation - in production, use system APIs
        // This could use /proc/meminfo on Linux or similar mechanisms
        let base = 1024 * 1024 * 512; // 512 MB base
        let variation = (chrono::Utc::now().timestamp() % (1024 * 1024 * 100)) as u64;
        base + variation
    }

    /// Get memory usage percentage
    async fn get_memory_percent() -> f64 {
        // Mock implementation
        let memory_bytes = Self::get_memory_usage().await;
        let total_memory = 1024 * 1024 * 1024 * 8; // Assume 8GB total
        (memory_bytes as f64 / total_memory as f64) * 100.0
    }

    /// Get disk usage in bytes
    async fn get_disk_usage() -> u64 {
        // Mock implementation
        let base = 1024 * 1024 * 1024 * 10; // 10 GB base
        let variation = (chrono::Utc::now().timestamp() % (1024 * 1024 * 1024)) as u64;
        base + variation
    }

    /// Get disk usage percentage
    async fn get_disk_percent() -> f64 {
        // Mock implementation
        let disk_bytes = Self::get_disk_usage().await;
        let total_disk = 1024 * 1024 * 1024 * 100; // Assume 100GB total
        (disk_bytes as f64 / total_disk as f64) * 100.0
    }

    /// Get network bytes received
    async fn get_network_rx() -> u64 {
        // Mock implementation
        (chrono::Utc::now().timestamp() * 1024) as u64
    }

    /// Get network bytes transmitted
    async fn get_network_tx() -> u64 {
        // Mock implementation
        (chrono::Utc::now().timestamp() * 512) as u64
    }

    /// Get number of open file descriptors
    async fn get_open_fds() -> u32 {
        // Mock implementation
        let base = 50;
        let variation = (chrono::Utc::now().timestamp() % 20) as u32;
        base + variation
    }

    /// Get resource usage summary
    pub async fn usage_summary(&self) -> ResourceSummary {
        let usage = self.current_usage().await;

        ResourceSummary {
            cpu_status: Self::classify_cpu_usage(usage.cpu_percent),
            memory_status: Self::classify_memory_usage(usage.memory_percent),
            disk_status: Self::classify_disk_usage(usage.disk_percent),
            overall_status: Self::classify_overall_status(&usage),
            timestamp: usage.timestamp,
        }
    }

    /// Classify CPU usage level
    fn classify_cpu_usage(cpu_percent: f64) -> ResourceStatus {
        if cpu_percent < 50.0 {
            ResourceStatus::Normal
        } else if cpu_percent < 80.0 {
            ResourceStatus::Warning
        } else {
            ResourceStatus::Critical
        }
    }

    /// Classify memory usage level
    fn classify_memory_usage(memory_percent: f64) -> ResourceStatus {
        if memory_percent < 70.0 {
            ResourceStatus::Normal
        } else if memory_percent < 90.0 {
            ResourceStatus::Warning
        } else {
            ResourceStatus::Critical
        }
    }

    /// Classify disk usage level
    fn classify_disk_usage(disk_percent: f64) -> ResourceStatus {
        if disk_percent < 80.0 {
            ResourceStatus::Normal
        } else if disk_percent < 95.0 {
            ResourceStatus::Warning
        } else {
            ResourceStatus::Critical
        }
    }

    /// Classify overall resource status
    fn classify_overall_status(usage: &ResourceUsage) -> ResourceStatus {
        let cpu_status = Self::classify_cpu_usage(usage.cpu_percent);
        let memory_status = Self::classify_memory_usage(usage.memory_percent);
        let disk_status = Self::classify_disk_usage(usage.disk_percent);

        // Return the worst status
        if cpu_status == ResourceStatus::Critical
            || memory_status == ResourceStatus::Critical
            || disk_status == ResourceStatus::Critical
        {
            ResourceStatus::Critical
        } else if cpu_status == ResourceStatus::Warning
            || memory_status == ResourceStatus::Warning
            || disk_status == ResourceStatus::Warning
        {
            ResourceStatus::Warning
        } else {
            ResourceStatus::Normal
        }
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource status classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceStatus {
    /// Resource usage is normal
    Normal,
    /// Resource usage is elevated but acceptable
    Warning,
    /// Resource usage is critically high
    Critical,
}

/// Resource usage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSummary {
    /// CPU status
    pub cpu_status: ResourceStatus,
    /// Memory status
    pub memory_status: ResourceStatus,
    /// Disk status
    pub disk_status: ResourceStatus,
    /// Overall status
    pub overall_status: ResourceStatus,
    /// Timestamp of the summary
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_resource_usage_default() {
        let usage = ResourceUsage::default();
        assert_eq!(usage.cpu_percent, 0.0);
        assert_eq!(usage.memory_bytes, 0);
        assert_eq!(usage.memory_percent, 0.0);
    }

    #[tokio::test]
    async fn test_resource_monitor_creation() {
        let monitor = ResourceMonitor::new();
        assert!(!monitor.is_running().await);
    }

    #[tokio::test]
    async fn test_resource_monitor_lifecycle() {
        let monitor = ResourceMonitor::new();

        // Test starting and stopping
        assert!(!monitor.is_running().await);

        let result = monitor.start().await;
        assert!(result.is_ok());
        assert!(monitor.is_running().await);

        let result = monitor.stop().await;
        assert!(result.is_ok());
        assert!(!monitor.is_running().await);
    }

    #[tokio::test]
    async fn test_current_usage() {
        let monitor = ResourceMonitor::new();
        let usage = monitor.current_usage().await;

        // Should return default values initially
        assert_eq!(usage.cpu_percent, 0.0);
        assert_eq!(usage.memory_bytes, 0);
    }

    #[test]
    fn test_resource_status_classification() {
        assert_eq!(
            ResourceMonitor::classify_cpu_usage(30.0),
            ResourceStatus::Normal
        );
        assert_eq!(
            ResourceMonitor::classify_cpu_usage(60.0),
            ResourceStatus::Warning
        );
        assert_eq!(
            ResourceMonitor::classify_cpu_usage(90.0),
            ResourceStatus::Critical
        );

        assert_eq!(
            ResourceMonitor::classify_memory_usage(50.0),
            ResourceStatus::Normal
        );
        assert_eq!(
            ResourceMonitor::classify_memory_usage(80.0),
            ResourceStatus::Warning
        );
        assert_eq!(
            ResourceMonitor::classify_memory_usage(95.0),
            ResourceStatus::Critical
        );

        assert_eq!(
            ResourceMonitor::classify_disk_usage(70.0),
            ResourceStatus::Normal
        );
        assert_eq!(
            ResourceMonitor::classify_disk_usage(85.0),
            ResourceStatus::Warning
        );
        assert_eq!(
            ResourceMonitor::classify_disk_usage(98.0),
            ResourceStatus::Critical
        );
    }

    #[tokio::test]
    async fn test_usage_summary() {
        let monitor = ResourceMonitor::new();
        let summary = monitor.usage_summary().await;

        // Should return normal status for default values
        assert_eq!(summary.cpu_status, ResourceStatus::Normal);
        assert_eq!(summary.memory_status, ResourceStatus::Normal);
        assert_eq!(summary.disk_status, ResourceStatus::Normal);
        assert_eq!(summary.overall_status, ResourceStatus::Normal);
    }

    #[test]
    fn test_overall_status_classification() {
        let usage_normal = ResourceUsage {
            cpu_percent: 30.0,
            memory_percent: 50.0,
            disk_percent: 60.0,
            ..Default::default()
        };
        assert_eq!(
            ResourceMonitor::classify_overall_status(&usage_normal),
            ResourceStatus::Normal
        );

        let usage_warning = ResourceUsage {
            cpu_percent: 60.0,
            memory_percent: 50.0,
            disk_percent: 60.0,
            ..Default::default()
        };
        assert_eq!(
            ResourceMonitor::classify_overall_status(&usage_warning),
            ResourceStatus::Warning
        );

        let usage_critical = ResourceUsage {
            cpu_percent: 90.0,
            memory_percent: 50.0,
            disk_percent: 60.0,
            ..Default::default()
        };
        assert_eq!(
            ResourceMonitor::classify_overall_status(&usage_critical),
            ResourceStatus::Critical
        );
    }

    #[test]
    fn test_resource_usage_setters() {
        let actual = ResourceUsage::default()
            .cpu_percent(75.5)
            .memory_bytes(1024u64 * 1024 * 1024) // 1GB
            .memory_percent(50.0)
            .disk_bytes(10u64 * 1024 * 1024 * 1024) // 10GB
            .disk_percent(80.0)
            .network_rx_bytes(1000u64)
            .network_tx_bytes(2000u64)
            .open_fds(100u32);

        assert_eq!(actual.cpu_percent, 75.5);
        assert_eq!(actual.memory_bytes, 1024u64 * 1024 * 1024);
        assert_eq!(actual.memory_percent, 50.0);
        assert_eq!(actual.disk_bytes, 10u64 * 1024 * 1024 * 1024);
        assert_eq!(actual.disk_percent, 80.0);
        assert_eq!(actual.network_rx_bytes, 1000u64);
        assert_eq!(actual.network_tx_bytes, 2000u64);
        assert_eq!(actual.open_fds, 100u32);
    }
}
