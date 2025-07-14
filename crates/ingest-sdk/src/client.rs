//! HTTP client for interacting with the Inngest API

use crate::error::{Result, SdkError};
use ingest_core::{Event, EventId, ExecutionId, Function, FunctionId, Json};
use reqwest::{Client, Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};
use url::Url;

/// Configuration for the Inngest client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API key for authentication
    api_key: String,
    /// Base URL for the Inngest API
    base_url: Url,
    /// Request timeout
    timeout: Duration,
    /// User agent string
    user_agent: String,
    /// Maximum number of retries
    max_retries: u32,
    /// Retry delay
    retry_delay: Duration,
}

impl ClientConfig {
    /// Create a new client configuration
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.inngest.com"
                .parse()
                .expect("Valid default URL"),
            timeout: Duration::from_secs(30),
            user_agent: format!("inngest-rust-sdk/{}", env!("CARGO_PKG_VERSION")),
            max_retries: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    /// Set the base URL
    pub fn base_url(mut self, url: impl AsRef<str>) -> Result<Self> {
        self.base_url = url.as_ref().parse()?;
        Ok(self)
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the user agent
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Get the base URL
    pub fn get_base_url(&self) -> &Url {
        &self.base_url
    }

    /// Get the timeout
    pub fn get_timeout(&self) -> Duration {
        self.timeout
    }

    /// Set the maximum number of retries
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the retry delay
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Get the API key
    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

/// HTTP client for the Inngest API
pub struct IngestClient {
    config: ClientConfig,
    client: Client,
}

impl IngestClient {
    /// Create a new Inngest client
    #[instrument(skip(config))]
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()?;

        info!("Created Inngest client with base URL: {}", config.base_url);

        Ok(Self { config, client })
    }

    /// Publish a single event
    #[instrument(skip(self, event))]
    pub async fn publish_event(&self, event: Event) -> Result<EventId> {
        debug!("Publishing event: {}", event.id);

        let response = self
            .request(Method::POST, "/v1/events")
            .json(&event)
            .send()
            .await?;

        let result: PublishEventResponse = self.handle_response(response).await?;

        info!("Published event: {}", result.event_id);
        Ok(result.event_id)
    }

    /// Publish multiple events
    #[instrument(skip(self, events))]
    pub async fn publish_events(&self, events: Vec<Event>) -> Result<Vec<EventId>> {
        debug!("Publishing {} events", events.len());

        let request = PublishEventsRequest { events };
        let response = self
            .request(Method::POST, "/v1/events/batch")
            .json(&request)
            .send()
            .await?;

        let result: PublishEventsResponse = self.handle_response(response).await?;

        info!("Published {} events", result.event_ids.len());
        Ok(result.event_ids)
    }

    /// Get a function by ID
    #[instrument(skip(self))]
    pub async fn get_function(&self, id: FunctionId) -> Result<Function> {
        debug!("Getting function: {}", id);

        let response = self
            .request(Method::GET, &format!("/v1/functions/{id}"))
            .send()
            .await?;

        let function = self.handle_response(response).await?;

        info!("Retrieved function: {}", id);
        Ok(function)
    }

    /// Invoke a function
    #[instrument(skip(self, input))]
    pub async fn invoke_function(&self, id: FunctionId, input: Json) -> Result<ExecutionId> {
        debug!("Invoking function: {}", id);

        let request = InvokeFunctionRequest { input };
        let response = self
            .request(Method::POST, &format!("/v1/functions/{id}/invoke"))
            .json(&request)
            .send()
            .await?;

        let result: InvokeFunctionResponse = self.handle_response(response).await?;

        info!("Invoked function {}: execution {}", id, result.execution_id);
        Ok(result.execution_id)
    }

    /// Get execution details
    #[instrument(skip(self))]
    pub async fn get_execution(&self, id: ExecutionId) -> Result<ExecutionDetails> {
        debug!("Getting execution: {}", id);

        let response = self
            .request(Method::GET, &format!("/v1/executions/{id}"))
            .send()
            .await?;

        let execution = self.handle_response(response).await?;

        info!("Retrieved execution: {}", id);
        Ok(execution)
    }

    /// Cancel an execution
    #[instrument(skip(self))]
    pub async fn cancel_execution(&self, id: ExecutionId) -> Result<()> {
        debug!("Cancelling execution: {}", id);

        let response = self
            .request(Method::POST, &format!("/v1/executions/{id}/cancel"))
            .send()
            .await?;

        self.handle_response::<()>(response).await?;

        info!("Cancelled execution: {}", id);
        Ok(())
    }

    /// Create a request builder
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = self.config.base_url.join(path).expect("Valid URL");

        self.client
            .request(method, url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
    }

    /// Handle API response with error handling and retries
    async fn handle_response<T>(&self, response: Response) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let status = response.status();

        if status.is_success() {
            if status == 204 {
                // No content response
                return Ok(serde_json::from_str("null")?);
            }

            let text = response.text().await?;
            match serde_json::from_str(&text) {
                Ok(data) => Ok(data),
                Err(err) => {
                    error!("Failed to parse response: {}", text);
                    Err(SdkError::Serialization(err.to_string()))
                }
            }
        } else {
            let text = response.text().await.unwrap_or_default();

            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&text) {
                warn!("API error: {} - {}", status, error_response.message);
                Err(SdkError::api(status.as_u16(), error_response.message))
            } else {
                warn!("API error: {} - {}", status, text);
                Err(SdkError::api(status.as_u16(), text))
            }
        }
    }
}

/// Request/Response types for API communication

#[derive(Debug, Serialize)]
struct PublishEventsRequest {
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct PublishEventResponse {
    event_id: EventId,
}

#[derive(Debug, Deserialize)]
struct PublishEventsResponse {
    event_ids: Vec<EventId>,
}

#[derive(Debug, Serialize)]
struct InvokeFunctionRequest {
    input: Json,
}

#[derive(Debug, Deserialize)]
struct InvokeFunctionResponse {
    execution_id: ExecutionId,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionDetails {
    pub id: ExecutionId,
    pub function_id: FunctionId,
    pub status: String,
    pub input: Json,
    pub output: Option<Json>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_client_config_new() {
        let fixture = ClientConfig::new("test-key");
        let actual = fixture.api_key();
        let expected = "test-key";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_client_config_base_url() {
        let fixture = ClientConfig::new("test-key")
            .base_url("https://custom.example.com")
            .unwrap();
        let actual = fixture.get_base_url().as_str();
        let expected = "https://custom.example.com/";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_client_config_timeout() {
        let fixture = ClientConfig::new("test-key").timeout(Duration::from_secs(60));
        let actual = fixture.get_timeout();
        let expected = Duration::from_secs(60);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_client_config_max_retries() {
        let fixture = ClientConfig::new("test-key").max_retries(5);
        let actual = fixture.max_retries;
        let expected = 5;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_client_creation() {
        let fixture = ClientConfig::new("test-key");
        let actual = IngestClient::new(fixture).await;
        assert!(actual.is_ok());
    }
}
