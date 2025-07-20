use thiserror::Error;

/// Core error types for Inngest
#[derive(Error, Debug)]
pub enum InngestError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Function not found: {id}")]
    FunctionNotFound { id: String },
    
    #[error("Event not found: {id}")]
    EventNotFound { id: String },
    
    #[error("Run not found: {id}")]
    RunNotFound { id: String },
    
    #[error("Step not found: {id}")]
    StepNotFound { id: String },
    
    #[error("Function execution failed: {reason}")]
    ExecutionFailed { reason: String },
    
    #[error("Rate limit exceeded: {reason}")]
    RateLimitExceeded { reason: String },
    
    #[error("Concurrency limit exceeded: {reason}")]
    ConcurrencyLimitExceeded { reason: String },
    
    #[error("Invalid configuration: {reason}")]
    InvalidConfiguration { reason: String },
    
    #[error("Step timeout: {step_id}")]
    StepTimeout { step_id: String },
    
    #[error("Function timeout: {function_id}")]
    FunctionTimeout { function_id: String },
    
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
    
    #[error("Database error: {message}")]
    Database { message: String },
    
    #[error("Redis error: {message}")]
    Redis { message: String },
    
    #[error("HTTP error: {message}")]
    Http { message: String },
}

pub type Result<T> = std::result::Result<T, InngestError>;
