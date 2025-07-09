use derive_setters::Setters;
use serde::{Deserialize, Serialize};

/// Unified queue configuration structure combining all subsystem configurations
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct QueueConfig {
    // Core queue configuration
    pub max_concurrent_jobs: u32,
    pub max_retries: u32,
    pub default_priority: u8,
    pub health_check_interval: u64,
    pub cleanup_interval: u64,

    // Scheduler configuration
    pub fair_scheduling: bool,
    pub max_jobs_per_tenant: u32,
    pub priority_weights: PriorityWeights,

    // Flow control configuration
    pub enable_backpressure: bool,
    pub max_queue_size: usize,
    pub rate_limit_per_second: u32,

    // Tenant configuration
    pub tenant_isolation: bool,
    pub quota_enforcement: bool,
    pub max_tenants: u32,

    // Storage configuration
    pub enable_cache: bool,
    pub cache_ttl_seconds: u64,
    pub write_through: bool,
    pub read_through: bool,

    // Dead letter configuration
    pub dead_letter_enabled: bool,
    pub dead_letter_max_age_hours: u64,
    pub dead_letter_cleanup_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityWeights {
    pub critical: f64,
    pub high: f64,
    pub normal: f64,
    pub low: f64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            // Core configuration
            max_concurrent_jobs: 100,
            max_retries: 3,
            default_priority: 2, // normal
            health_check_interval: 30,
            cleanup_interval: 300,

            // Scheduler configuration
            fair_scheduling: true,
            max_jobs_per_tenant: 50,
            priority_weights: PriorityWeights::default(),

            // Flow control configuration
            enable_backpressure: true,
            max_queue_size: 10000,
            rate_limit_per_second: 1000,

            // Tenant configuration
            tenant_isolation: true,
            quota_enforcement: true,
            max_tenants: 1000,

            // Storage configuration
            enable_cache: true,
            cache_ttl_seconds: 3600,
            write_through: true,
            read_through: true,

            // Dead letter configuration
            dead_letter_enabled: true,
            dead_letter_max_age_hours: 24,
            dead_letter_cleanup_interval: 3600,
        }
    }
}

impl Default for PriorityWeights {
    fn default() -> Self {
        Self {
            critical: 4.0,
            high: 3.0,
            normal: 2.0,
            low: 1.0,
        }
    }
}

impl QueueConfig {
    /// Returns scheduler-related configuration settings
    pub fn scheduler_config(&self) -> SchedulerConfig {
        SchedulerConfig {
            fair_scheduling: self.fair_scheduling,
            max_jobs_per_tenant: self.max_jobs_per_tenant,
            priority_weights: self.priority_weights.clone(),
        }
    }

    /// Returns flow control related configuration settings
    pub fn flow_control_config(&self) -> FlowControlConfig {
        FlowControlConfig {
            enable_backpressure: self.enable_backpressure,
            max_queue_size: self.max_queue_size,
            rate_limit_per_second: self.rate_limit_per_second,
        }
    }

    /// Returns storage-related configuration settings
    pub fn storage_config(&self) -> StorageConfig {
        StorageConfig {
            enable_cache: self.enable_cache,
            cache_ttl_seconds: self.cache_ttl_seconds,
            write_through: self.write_through,
            read_through: self.read_through,
        }
    }
}

/// Simplified scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub fair_scheduling: bool,
    pub max_jobs_per_tenant: u32,
    pub priority_weights: PriorityWeights,
}

/// Simplified flow control configuration
#[derive(Debug, Clone)]
pub struct FlowControlConfig {
    pub enable_backpressure: bool,
    pub max_queue_size: usize,
    pub rate_limit_per_second: u32,
}

/// Simplified storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub enable_cache: bool,
    pub cache_ttl_seconds: u64,
    pub write_through: bool,
    pub read_through: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_queue_config_default() {
        let config = QueueConfig::default();

        assert_eq!(config.max_concurrent_jobs, 100);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.fair_scheduling, true);
        assert_eq!(config.enable_cache, true);
        assert_eq!(config.dead_letter_enabled, true);
    }

    #[test]
    fn test_config_extraction() {
        let config = QueueConfig::default();

        let scheduler_config = config.scheduler_config();
        assert_eq!(scheduler_config.fair_scheduling, true);
        assert_eq!(scheduler_config.max_jobs_per_tenant, 50);

        let flow_config = config.flow_control_config();
        assert_eq!(flow_config.enable_backpressure, true);
        assert_eq!(flow_config.max_queue_size, 10000);

        let storage_config = config.storage_config();
        assert_eq!(storage_config.enable_cache, true);
        assert_eq!(storage_config.write_through, true);
    }

    #[test]
    fn test_priority_weights_default() {
        let weights = PriorityWeights::default();

        assert_eq!(weights.critical, 4.0);
        assert_eq!(weights.high, 3.0);
        assert_eq!(weights.normal, 2.0);
        assert_eq!(weights.low, 1.0);
    }
}
