//! Local development server for testing functions

use crate::error::{Result, SdkError};
use crate::function::{Function, StepContext};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json as AxumJson,
    routing::{get, post},
};
use ingest_core::{Event, EventId, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Local development server for testing functions
pub struct LocalServer {
    /// Registered functions
    functions: Arc<RwLock<HashMap<String, Arc<dyn Function>>>>,
    /// Server port
    port: u16,
    /// Server host
    host: String,
}

impl LocalServer {
    /// Create a new local server
    pub fn new() -> Self {
        Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
            port: 3000,
            host: "127.0.0.1".to_string(),
        }
    }

    /// Set the server port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the server host
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Register a function
    #[instrument(skip(self, function))]
    pub async fn register_function<F: Function + 'static>(&self, function: F) {
        let name = function.name().to_string();
        info!("Registering function: {}", name);

        let mut functions = self.functions.write().await;
        functions.insert(name.clone(), Arc::new(function));

        debug!("Function {} registered successfully", name);
    }

    /// Start the local server
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        let app = self.create_app().await;
        let addr: SocketAddr = format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|e| SdkError::local_server(format!("Invalid address: {e}")))?;

        info!("Starting local server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| SdkError::local_server(format!("Failed to bind to {addr}: {e}")))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| SdkError::local_server(format!("Server error: {e}")))?;

        Ok(())
    }

    /// Trigger an event locally
    #[instrument(skip(self, event))]
    pub async fn trigger_event(&self, event: Event) -> Result<Vec<LocalExecutionResult>> {
        debug!("Triggering event: {}", event.name());

        let functions = self.functions.read().await;
        let mut results = Vec::new();

        for (name, function) in functions.iter() {
            // Check if function should be triggered by this event
            if self.should_trigger_function(function.as_ref(), &event) {
                info!("Triggering function {} for event {}", name, event.name());

                let result = self.execute_function(function.clone(), event.clone()).await;
                results.push(LocalExecutionResult {
                    function_name: name.clone(),
                    event_id: event.id.clone(),
                    result,
                });
            }
        }

        if results.is_empty() {
            warn!("No functions triggered for event: {}", event.name());
        }

        Ok(results)
    }

    /// Get registered functions
    pub async fn get_functions(&self) -> Vec<String> {
        let functions = self.functions.read().await;
        functions.keys().cloned().collect()
    }

    /// Create the Axum app
    async fn create_app(&self) -> Router {
        let state = LocalServerState {
            functions: self.functions.clone(),
        };

        Router::new()
            .route("/", get(health_check))
            .route("/functions", get(list_functions))
            .route("/functions/:name", get(get_function))
            .route("/events", post(trigger_event))
            .route("/events/:name", post(trigger_named_event))
            .with_state(state)
    }

    /// Check if a function should be triggered by an event
    fn should_trigger_function(&self, function: &dyn Function, event: &Event) -> bool {
        let config = function.config();

        for trigger in &config.triggers {
            if self.matches_pattern(&trigger.event, event.name()) {
                // TODO: Evaluate expression filter if present
                return true;
            }
        }

        false
    }

    /// Check if an event name matches a pattern
    fn matches_pattern(&self, pattern: &str, event_name: &str) -> bool {
        // Simple pattern matching - in a real implementation, this would be more sophisticated
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            return event_name.starts_with(prefix);
        }

        pattern == event_name
    }

    /// Execute a function
    async fn execute_function(&self, function: Arc<dyn Function>, event: Event) -> Result<Json> {
        let mut step_context = crate::function::StepContextWrapper::Standard(StepContext::new());
        function.execute(event, &mut step_context).await
    }
}

impl Default for LocalServer {
    fn default() -> Self {
        Self::new()
    }
}

/// State for the local server
#[derive(Clone)]
struct LocalServerState {
    functions: Arc<RwLock<HashMap<String, Arc<dyn Function>>>>,
}

/// Result of a local function execution
#[derive(Debug, Serialize)]
pub struct LocalExecutionResult {
    /// Function name
    pub function_name: String,
    /// Event ID that triggered the execution
    pub event_id: EventId,
    /// Execution result
    pub result: Result<Json>,
}

/// Request/Response types for API endpoints

#[derive(Debug, Deserialize)]
struct TriggerEventRequest {
    event: Event,
}

#[derive(Debug, Serialize)]
struct TriggerEventResponse {
    results: Vec<LocalExecutionResult>,
}

#[derive(Debug, Serialize)]
struct ListFunctionsResponse {
    functions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GetFunctionResponse {
    name: String,
    config: serde_json::Value,
}

/// Health check endpoint
async fn health_check() -> AxumJson<serde_json::Value> {
    AxumJson(serde_json::json!({
        "status": "healthy",
        "service": "inngest-local-server",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// List all registered functions
async fn list_functions(
    State(state): State<LocalServerState>,
) -> std::result::Result<AxumJson<ListFunctionsResponse>, StatusCode> {
    let functions = state.functions.read().await;
    let function_names: Vec<String> = functions.keys().cloned().collect();

    Ok(AxumJson(ListFunctionsResponse {
        functions: function_names,
    }))
}

/// Get a specific function
async fn get_function(
    Path(name): Path<String>,
    State(state): State<LocalServerState>,
) -> std::result::Result<AxumJson<GetFunctionResponse>, StatusCode> {
    let functions = state.functions.read().await;

    if let Some(function) = functions.get(&name) {
        let config = serde_json::to_value(function.config())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(AxumJson(GetFunctionResponse { name, config }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Trigger an event
async fn trigger_event(
    State(state): State<LocalServerState>,
    AxumJson(request): AxumJson<TriggerEventRequest>,
) -> std::result::Result<AxumJson<TriggerEventResponse>, StatusCode> {
    let server = LocalServer {
        functions: state.functions,
        port: 0,             // Not used in this context
        host: String::new(), // Not used in this context
    };

    let results = server
        .trigger_event(request.event)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(AxumJson(TriggerEventResponse { results }))
}

/// Trigger a named event with data
async fn trigger_named_event(
    Path(name): Path<String>,
    State(state): State<LocalServerState>,
    AxumJson(data): AxumJson<Json>,
) -> std::result::Result<AxumJson<TriggerEventResponse>, StatusCode> {
    let event = Event::new(&name, data);

    let server = LocalServer {
        functions: state.functions,
        port: 0,             // Not used in this context
        host: String::new(), // Not used in this context
    };

    let results = server
        .trigger_event(event)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(AxumJson(TriggerEventResponse { results }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::{FunctionConfig, FunctionTrigger, StepContextWrapper};
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;

    struct TestFunction {
        name: String,
        config: FunctionConfig,
    }

    impl TestFunction {
        fn new(name: &str, trigger: &str) -> Self {
            let config = FunctionConfig::new(name).trigger(FunctionTrigger::event(trigger));

            Self {
                name: name.to_string(),
                config,
            }
        }
    }

    #[async_trait]
    impl Function for TestFunction {
        fn name(&self) -> &str {
            &self.name
        }

        fn config(&self) -> &FunctionConfig {
            &self.config
        }

        async fn execute(&self, _event: Event, _step: &mut StepContextWrapper) -> Result<Json> {
            Ok(serde_json::json!({"result": "success"}))
        }
    }

    #[test]
    fn test_local_server_new() {
        let fixture = LocalServer::new();
        assert_eq!(fixture.port, 3000);
        assert_eq!(fixture.host, "127.0.0.1");
    }

    #[test]
    fn test_local_server_port() {
        let fixture = LocalServer::new().port(8080);
        assert_eq!(fixture.port, 8080);
    }

    #[test]
    fn test_local_server_host() {
        let fixture = LocalServer::new().host("0.0.0.0");
        assert_eq!(fixture.host, "0.0.0.0");
    }

    #[tokio::test]
    async fn test_register_function() {
        let fixture = LocalServer::new();
        let function = TestFunction::new("test-function", "test.event");

        fixture.register_function(function).await;

        let functions = fixture.get_functions().await;
        assert_eq!(functions.len(), 1);
        assert!(functions.contains(&"test-function".to_string()));
    }

    #[tokio::test]
    async fn test_trigger_event() {
        let fixture = LocalServer::new();
        let function = TestFunction::new("test-function", "test.event");

        fixture.register_function(function).await;

        let event = Event::new("test.event", serde_json::json!({"key": "value"}));

        let actual = fixture.trigger_event(event).await.unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].function_name, "test-function");
    }

    #[test]
    fn test_matches_pattern() {
        let fixture = LocalServer::new();

        // Exact match
        assert!(fixture.matches_pattern("user.created", "user.created"));

        // Wildcard match
        assert!(fixture.matches_pattern("user.*", "user.created"));
        assert!(fixture.matches_pattern("user.*", "user.updated"));

        // No match
        assert!(!fixture.matches_pattern("user.created", "order.created"));
    }
}
