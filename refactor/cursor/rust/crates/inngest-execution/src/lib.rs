//! Execution engine for Inngest functions
//!
//! This crate handles the core execution logic including:
//! - Function execution orchestration
//! - Batch processing
//! - Concurrency control
//! - Step execution and state management
//! - Retry and error handling

pub mod batch;
pub mod concurrency;
pub mod driver;
pub mod executor;
// TODO: Implement these modules
// pub mod lifecycle;
// pub mod pauses;
// pub mod runner;
// pub mod history;

// Re-export commonly used types
pub use batch::{BatchConfig, BatchItem, BatchManager};
pub use concurrency::{ConcurrencyLimits, ConcurrencyManager};
pub use driver::{ConnectDriver, ExecutionDriver, HttpDriver};
pub use executor::{ExecutionRequest, Executor, ExecutorConfig};
pub use inngest_core::{Error, Result};

/// Default configuration values
pub mod defaults {
    use std::time::Duration;

    /// Default maximum retry count
    pub const DEFAULT_RETRY_COUNT: u32 = 3;

    /// Default step timeout
    pub const DEFAULT_STEP_TIMEOUT: Duration = Duration::from_secs(60);

    /// Default batch size limit
    pub const DEFAULT_BATCH_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10MB

    /// Default concurrent executions
    pub const DEFAULT_CONCURRENCY_LIMIT: usize = 100;
}
