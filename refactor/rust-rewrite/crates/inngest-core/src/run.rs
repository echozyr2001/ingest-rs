use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A function run represents a single execution of a function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    /// Unique identifier for this run
    pub id: String,
    
    /// ID of the function being executed
    pub function_id: String,
    
    /// ID of the event that triggered this run
    pub event_id: String,
    
    /// Current status of the run
    pub status: String,
    
    /// When the run started
    pub started_at: Option<DateTime<Utc>>,
    
    /// When the run ended
    pub ended_at: Option<DateTime<Utc>>,
    
    /// Output from the function execution
    pub output: Option<serde_json::Value>,
    
    /// Steps executed during this run
    pub steps: Vec<RunStep>,
}

/// A step within a function run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStep {
    /// Step identifier
    pub id: String,
    
    /// Step name
    pub name: String,
    
    /// Step status
    pub status: String,
    
    /// Input to the step
    pub input: Option<serde_json::Value>,
    
    /// Output from the step
    pub output: Option<serde_json::Value>,
    
    /// Error message if the step failed
    pub error_message: Option<String>,
    
    /// When the step started
    pub started_at: Option<DateTime<Utc>>,
    
    /// When the step ended
    pub ended_at: Option<DateTime<Utc>>,
}

impl Run {
    /// Create a new run in queued status
    pub fn new(id: String, function_id: String, event_id: String) -> Self {
        Self {
            id,
            function_id,
            event_id,
            status: "queued".to_string(),
            started_at: None,
            ended_at: None,
            output: None,
            steps: vec![],
        }
    }
    
    /// Mark the run as started
    pub fn start(&mut self) {
        self.status = "running".to_string();
        self.started_at = Some(Utc::now());
    }
    
    /// Mark the run as completed successfully
    pub fn complete(&mut self, output: Option<serde_json::Value>) {
        self.status = "completed".to_string();
        self.ended_at = Some(Utc::now());
        self.output = output;
    }
    
    /// Mark the run as failed
    pub fn fail(&mut self, error: String) {
        self.status = "failed".to_string();
        self.ended_at = Some(Utc::now());
        self.output = Some(serde_json::json!({
            "error": error
        }));
    }
}

impl RunStep {
    /// Create a new step
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            status: "pending".to_string(),
            input: None,
            output: None,
            error_message: None,
            started_at: None,
            ended_at: None,
        }
    }
    
    /// Start the step
    pub fn start(&mut self, input: Option<serde_json::Value>) {
        self.status = "running".to_string();
        self.started_at = Some(Utc::now());
        self.input = input;
    }
    
    /// Complete the step successfully
    pub fn complete(&mut self, output: Option<serde_json::Value>) {
        self.status = "completed".to_string();
        self.ended_at = Some(Utc::now());
        self.output = output;
    }
    
    /// Fail the step
    pub fn fail(&mut self, error: String) {
        self.status = "failed".to_string();
        self.ended_at = Some(Utc::now());
        self.error_message = Some(error);
    }
}
