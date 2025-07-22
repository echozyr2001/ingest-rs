//! Execution drivers for different transport protocols
//!
//! Provides abstraction over HTTP and Connect (WebSocket) protocols for function execution

use async_trait::async_trait;
use inngest_core::{Error, ExecutionContext, Result};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

/// Request payload sent to function endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    /// Function identifier
    pub function_id: String,
    /// Step identifier (optional)
    pub step_id: Option<String>,
    /// Execution context
    pub ctx: ExecutionContext,
    /// Event data or step data
    pub event: serde_json::Value,
    /// Previous step outputs (for multi-step functions)
    pub steps: HashMap<String, serde_json::Value>,
}

/// Response from function execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    /// Status of the execution
    pub status: ExecutionStatus,
    /// Response body/data
    pub body: Option<serde_json::Value>,
    /// Retry configuration
    pub retry_after: Option<Duration>,
    /// Whether to retry this execution
    pub no_retry: bool,
    /// Next steps to execute
    pub steps: Vec<StepRequest>,
}

/// Status of function execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution is waiting (paused)
    Waiting,
    /// Execution was cancelled
    Cancelled,
}

/// Request for a step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRequest {
    pub id: String,
    pub name: String,
    pub data: serde_json::Value,
}

/// Configuration for execution drivers
#[derive(Debug, Clone)]
pub struct DriverConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub user_agent: String,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_retries: 3,
            user_agent: "inngest-rust/0.1.0".to_string(),
        }
    }
}

/// Trait for execution drivers
#[async_trait]
pub trait ExecutionDriver: Send + Sync {
    /// Execute a function step
    async fn execute(&self, url: &Url, request: ExecutionRequest) -> Result<ExecutionResponse>;

    /// Get the driver name
    fn name(&self) -> &'static str;

    /// Check if the driver supports the given URL scheme
    fn supports_scheme(&self, scheme: &str) -> bool;
}

/// HTTP-based execution driver
pub struct HttpDriver {
    client: Client,
    config: DriverConfig,
}

impl HttpDriver {
    pub fn new(config: DriverConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| Error::Network(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, config })
    }

    async fn send_request(
        &self,
        url: &Url,
        request: ExecutionRequest,
    ) -> Result<ExecutionResponse> {
        let mut attempt = 0;

        loop {
            let response = self
                .client
                .request(Method::POST, url.clone())
                .header("Content-Type", "application/json")
                .header("User-Agent", &self.config.user_agent)
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::Network(format!("HTTP request failed: {}", e)))?;

            let status_code = response.status();

            if status_code.is_success() {
                let exec_response: ExecutionResponse = response
                    .json()
                    .await
                    .map_err(|e| Error::Network(format!("Failed to parse response: {}", e)))?;

                return Ok(exec_response);
            }

            // Handle retryable errors
            if status_code.is_server_error() && attempt < self.config.max_retries {
                attempt += 1;
                let delay = Duration::from_millis(100 * (1 << attempt)); // Exponential backoff
                tokio::time::sleep(delay).await;
                continue;
            }

            // Handle client errors or max retries exceeded
            let error_body = response.text().await.unwrap_or_default();
            return Err(Error::Execution(
                inngest_core::error::ExecutionError::Runtime(format!(
                    "HTTP request failed with status {}: {}",
                    status_code, error_body
                )),
            ));
        }
    }
}

#[async_trait]
impl ExecutionDriver for HttpDriver {
    async fn execute(&self, url: &Url, request: ExecutionRequest) -> Result<ExecutionResponse> {
        tracing::debug!("Executing HTTP request to: {}", url);
        self.send_request(url, request).await
    }

    fn name(&self) -> &'static str {
        "http"
    }

    fn supports_scheme(&self, scheme: &str) -> bool {
        matches!(scheme, "http" | "https")
    }
}

/// Connect (WebSocket) based execution driver
pub struct ConnectDriver {
    config: DriverConfig,
}

impl ConnectDriver {
    pub fn new(config: DriverConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ExecutionDriver for ConnectDriver {
    async fn execute(&self, url: &Url, _request: ExecutionRequest) -> Result<ExecutionResponse> {
        tracing::debug!("Executing Connect request to: {}", url);

        // In a full implementation, this would use the Connect protocol
        // to communicate with the SDK over WebSocket

        // For now, return a placeholder response
        tracing::warn!("Connect driver not fully implemented");

        Ok(ExecutionResponse {
            status: ExecutionStatus::Failed,
            body: Some(serde_json::json!({
                "error": "Connect driver not implemented"
            })),
            retry_after: None,
            no_retry: true,
            steps: vec![],
        })
    }

    fn name(&self) -> &'static str {
        "connect"
    }

    fn supports_scheme(&self, scheme: &str) -> bool {
        matches!(scheme, "ws" | "wss")
    }
}

/// Multi-protocol execution driver that delegates to appropriate drivers
pub struct MultiDriver {
    drivers: HashMap<String, Box<dyn ExecutionDriver>>,
}

impl MultiDriver {
    pub fn new() -> Self {
        Self {
            drivers: HashMap::new(),
        }
    }

    pub fn register_driver(mut self, driver: Box<dyn ExecutionDriver>) -> Self {
        self.drivers.insert(driver.name().to_string(), driver);
        self
    }

    pub fn with_http(self, config: DriverConfig) -> Result<Self> {
        let driver = HttpDriver::new(config)?;
        Ok(self.register_driver(Box::new(driver)))
    }

    pub fn with_connect(self, config: DriverConfig) -> Self {
        let driver = ConnectDriver::new(config);
        self.register_driver(Box::new(driver))
    }

    fn get_driver_for_scheme(&self, scheme: &str) -> Option<&dyn ExecutionDriver> {
        self.drivers
            .values()
            .find(|driver| driver.supports_scheme(scheme))
            .map(|driver| driver.as_ref())
    }
}

impl Default for MultiDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionDriver for MultiDriver {
    async fn execute(&self, url: &Url, request: ExecutionRequest) -> Result<ExecutionResponse> {
        let driver = self.get_driver_for_scheme(url.scheme()).ok_or_else(|| {
            Error::InvalidConfiguration(format!("Unsupported URL scheme: {}", url.scheme()))
        })?;

        driver.execute(url, request).await
    }

    fn name(&self) -> &'static str {
        "multi"
    }

    fn supports_scheme(&self, scheme: &str) -> bool {
        self.get_driver_for_scheme(scheme).is_some()
    }
}

/// Determine driver type from URL scheme
pub fn scheme_to_driver(scheme: &str) -> &'static str {
    match scheme {
        "http" | "https" => "http",
        "ws" | "wss" => "connect",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inngest_core::{ExecutionContext, StateIdentifier};

    fn create_test_request() -> ExecutionRequest {
        ExecutionRequest {
            function_id: "test-function".to_string(),
            step_id: Some("step-1".to_string()),
            ctx: ExecutionContext {
                function_id: "test-function".to_string(),
                run_id: "test-run".to_string(),
                step_id: "step-1".to_string(),
                attempt: 0,
                state: StateIdentifier::new("test-function".to_string(), "test-run".to_string()),
            },
            event: serde_json::json!({"test": "data"}),
            steps: HashMap::new(),
        }
    }

    #[test]
    fn test_scheme_to_driver() {
        assert_eq!(scheme_to_driver("http"), "http");
        assert_eq!(scheme_to_driver("https"), "http");
        assert_eq!(scheme_to_driver("ws"), "connect");
        assert_eq!(scheme_to_driver("wss"), "connect");
        assert_eq!(scheme_to_driver("unknown"), "");
    }

    #[test]
    fn test_driver_supports_scheme() {
        let config = DriverConfig::default();
        let http_driver = HttpDriver::new(config.clone()).unwrap();
        let connect_driver = ConnectDriver::new(config);

        assert!(http_driver.supports_scheme("http"));
        assert!(http_driver.supports_scheme("https"));
        assert!(!http_driver.supports_scheme("ws"));

        assert!(connect_driver.supports_scheme("ws"));
        assert!(connect_driver.supports_scheme("wss"));
        assert!(!connect_driver.supports_scheme("http"));
    }

    #[test]
    fn test_multi_driver_setup() {
        let config = DriverConfig::default();
        let multi_driver = MultiDriver::new()
            .with_http(config.clone())
            .unwrap()
            .with_connect(config);

        assert!(multi_driver.supports_scheme("http"));
        assert!(multi_driver.supports_scheme("https"));
        assert!(multi_driver.supports_scheme("ws"));
        assert!(multi_driver.supports_scheme("wss"));
        assert!(!multi_driver.supports_scheme("ftp"));
    }

    #[tokio::test]
    async fn test_execution_request_serialization() {
        let request = create_test_request();
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ExecutionRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.function_id, deserialized.function_id);
        assert_eq!(request.step_id, deserialized.step_id);
    }

    #[tokio::test]
    async fn test_execution_response_serialization() {
        let response = ExecutionResponse {
            status: ExecutionStatus::Completed,
            body: Some(serde_json::json!({"result": "success"})),
            retry_after: Some(Duration::from_secs(30)),
            no_retry: false,
            steps: vec![],
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ExecutionResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.status, deserialized.status);
        assert_eq!(response.no_retry, deserialized.no_retry);
    }
}
