use crate::event::Event;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Function trigger types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    Event {
        event: String,
        expression: Option<String>,
    },
    Cron {
        cron: String,
    },
}

/// Function configuration for concurrency control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concurrency {
    /// Expression to generate concurrency key
    pub key: Option<String>,
    /// Maximum concurrent executions
    pub limit: u32,
    /// Scope of concurrency control
    pub scope: Option<ConcurrencyScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConcurrencyScope {
    Account,
    Environment,
    Function,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum requests per period
    pub limit: u32,
    /// Time period in seconds
    pub period: u32,
    /// Key for rate limiting
    pub key: Option<String>,
}

/// Batching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    /// Maximum batch size
    pub max_size: u32,
    /// Maximum wait time before processing partial batch
    pub timeout: std::time::Duration,
    /// Key for batching events together
    pub key: Option<String>,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retry {
    /// Maximum number of attempts
    pub attempts: u32,
}

/// Function configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionConfig {
    /// Function ID
    pub id: String,
    /// Function name
    pub name: String,
    /// Function triggers
    pub triggers: Vec<Trigger>,
    /// Concurrency control
    pub concurrency: Option<Concurrency>,
    /// Rate limiting
    pub rate_limit: Option<RateLimit>,
    /// Batching configuration
    pub batch: Option<Batch>,
    /// Retry configuration
    pub retry: Option<Retry>,
    /// Function steps/workflow definition
    pub steps: Vec<Step>,
}

/// Represents a step in a function workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Step ID
    pub id: String,
    /// Step name
    pub name: Option<String>,
    /// Step runtime configuration
    pub runtime: StepRuntime,
    /// Retry configuration for this step
    pub retry: Option<Retry>,
}

/// Step runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepRuntime {
    /// Run code step
    Run {
        /// Step handler name/identifier
        handler: String,
    },
    /// Sleep/wait step
    Sleep {
        /// Duration to sleep
        duration: std::time::Duration,
    },
    /// Wait for event step
    WaitForEvent {
        /// Event name to wait for
        event: String,
        /// Expression to match events
        expression: Option<String>,
        /// Timeout for waiting
        timeout: Option<std::time::Duration>,
    },
    /// Invoke another function
    InvokeFunction {
        /// Function ID to invoke
        function_id: String,
        /// Data to pass to invoked function
        data: Option<serde_json::Value>,
    },
}

/// Inngest function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Unique function identifier
    pub id: Uuid,
    /// Function configuration
    pub config: FunctionConfig,
    /// Function version
    pub version: u32,
    /// Application ID this function belongs to
    pub app_id: Uuid,
    /// Function slug for URL routing
    pub slug: String,
}

impl Function {
    /// Get function slug
    pub fn get_slug(&self) -> &str {
        &self.slug
    }

    /// Check if function has batch configuration
    pub fn is_batch_enabled(&self) -> bool {
        self.config.batch.is_some()
    }

    /// Get function triggers
    pub fn get_triggers(&self) -> &[Trigger] {
        &self.config.triggers
    }

    /// Check if function matches event
    pub fn matches_event(&self, event: &Event) -> bool {
        self.config.triggers.iter().any(|trigger| {
            match trigger {
                Trigger::Event {
                    event: event_name,
                    expression,
                } => {
                    if event.name == *event_name {
                        if let Some(_expr) = expression {
                            // TODO: Implement expression evaluation
                            true
                        } else {
                            true
                        }
                    } else {
                        false
                    }
                }
                Trigger::Cron { .. } => false,
            }
        })
    }
}

/// Function loader trait for retrieving function definitions
#[async_trait::async_trait]
pub trait FunctionLoader {
    type Error;

    /// Load function by ID
    async fn load_function(
        &self,
        env_id: Uuid,
        fn_id: Uuid,
    ) -> Result<Option<Function>, Self::Error>;

    /// Get all functions
    async fn get_functions(&self) -> Result<Vec<Function>, Self::Error>;

    /// Get functions by trigger event name
    async fn get_functions_by_trigger(
        &self,
        event_name: &str,
    ) -> Result<Vec<Function>, Self::Error>;

    /// Get scheduled functions (cron-triggered)
    async fn get_scheduled_functions(&self) -> Result<Vec<Function>, Self::Error>;
}
