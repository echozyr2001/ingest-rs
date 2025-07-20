use crate::{Event, Function, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub run_id: Uuid,
    pub function_id: String,
    pub event: Event,
    pub attempt: u32,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub retry_after: Option<chrono::Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed(ExecutionResult),
    Failed(String),
    Cancelled,
}

pub trait Executor: Send + Sync {
    async fn execute(
        &self,
        function: &Function,
        context: ExecutionContext,
    ) -> Result<ExecutionResult>;
}

pub struct HttpExecutor {
    client: reqwest::Client,
    timeout: std::time::Duration,
    base_url: String,
}

impl HttpExecutor {
    pub fn new(timeout: std::time::Duration, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            timeout,
            base_url,
        }
    }

    /// Extract retry information from function response
    fn extract_retry_after(&self, response: &serde_json::Value) -> Option<chrono::Duration> {
        // Check if response contains retry information
        if let Some(retry_obj) = response.get("retry") {
            if let Some(after_seconds) = retry_obj.get("after").and_then(|v| v.as_u64()) {
                return Some(chrono::Duration::seconds(after_seconds as i64));
            }
        }

        // Check for no-retry indicator
        if response
            .get("no_retry")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return None;
        }

        None
    }
}

impl Executor for HttpExecutor {
    async fn execute(
        &self,
        function: &Function,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // Prepare the Inngest standard payload format
        let payload = serde_json::json!({
            "event": context.event,
            "events": [context.event], // SDK expects events array
            "ctx": {
                "run_id": context.run_id.to_string(),
                "attempt": context.attempt,
                "fn_id": function.id.to_string(),
                "step_id": null, // Will be set for step functions
                "env": "dev", // TODO: Make configurable
                "metadata": context.metadata
            },
            "steps": {}, // TODO: Implement step tracking
            "use_api": false
        });

        // Construct URL using base_url and function slug
        let function_url = format!("{}/api/inngest", self.base_url);

        let response = self
            .client
            .post(&function_url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "inngest-rust/0.1.0")
            .header("X-Inngest-Framework", "rust")
            .header("X-Inngest-SDK", "rust-0.1.0")
            .json(&payload)
            .timeout(self.timeout)
            .send()
            .await;

        match response {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    // Try to parse the response
                    match response.json::<serde_json::Value>().await {
                        Ok(result) => {
                            // Check if the response indicates a retry
                            if let Some(retry_after) = self.extract_retry_after(&result) {
                                Ok(ExecutionResult {
                                    success: false,
                                    output: Some(result),
                                    error: None,
                                    retry_after: Some(retry_after),
                                })
                            } else {
                                Ok(ExecutionResult {
                                    success: true,
                                    output: Some(result),
                                    error: None,
                                    retry_after: None,
                                })
                            }
                        }
                        Err(e) => Ok(ExecutionResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to parse response: {}", e)),
                            retry_after: Some(chrono::Duration::seconds(30)),
                        }),
                    }
                } else if status.as_u16() == 206 {
                    // 206 means "retry"
                    let retry_after = chrono::Duration::seconds(30); // Default retry
                    Ok(ExecutionResult {
                        success: false,
                        output: None,
                        error: Some("Function requested retry".to_string()),
                        retry_after: Some(retry_after),
                    })
                } else {
                    // Error response
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| format!("HTTP {}", status));

                    Ok(ExecutionResult {
                        success: false,
                        output: None,
                        error: Some(error_text),
                        retry_after: Some(chrono::Duration::seconds(60)),
                    })
                }
            }
            Err(e) => {
                // Network or timeout error
                Ok(ExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Request failed: {}", e)),
                    retry_after: Some(chrono::Duration::seconds(120)),
                })
            }
        }
    }
}
