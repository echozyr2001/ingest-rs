use inngest_core::{EventRouter, Function, FunctionConfig, InngestError, Result, Trigger};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Function discovery service that finds and registers functions from URLs
#[derive(Clone)]
pub struct FunctionDiscovery {
    /// HTTP client for making discovery requests
    client: Client,
    /// Event router for registering discovered functions
    router: Arc<RwLock<EventRouter>>,
    /// Discovery configuration
    config: DiscoveryConfig,
    /// Cache of discovered functions
    function_cache: Arc<RwLock<Vec<DiscoveredFunction>>>,
}

/// Configuration for function discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// URLs to discover functions from
    pub discovery_urls: Vec<String>,
    /// Discovery request timeout
    pub timeout: Duration,
    /// Polling interval for continuous discovery
    pub poll_interval: Duration,
    /// Whether to enable continuous polling
    pub polling_enabled: bool,
    /// Request headers to include in discovery requests
    pub headers: std::collections::HashMap<String, String>,
}

/// A discovered function with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredFunction {
    /// The function definition
    pub function: Function,
    /// URL where the function was discovered
    pub source_url: String,
    /// Timestamp when discovered
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    /// Whether the function is currently active
    pub active: bool,
    /// Function metadata from the source
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Response from an Inngest SDK endpoint
#[derive(Debug, Deserialize)]
pub struct SdkResponse {
    /// Framework information
    pub framework: String,
    /// SDK version
    pub sdk: String,
    /// SDK configuration
    pub config: SdkConfig,
    /// List of functions
    pub functions: Vec<SdkFunction>,
}

/// SDK configuration
#[derive(Debug, Deserialize)]
pub struct SdkConfig {
    /// App ID
    pub app_id: Option<String>,
    /// Environment
    pub env: Option<String>,
    /// Event key
    pub event_key: Option<String>,
}

/// Function definition from SDK
#[derive(Debug, Deserialize)]
pub struct SdkFunction {
    /// Function ID
    pub id: String,
    /// Function name
    pub name: String,
    /// Function triggers
    pub triggers: Vec<SdkTrigger>,
    /// Function configuration
    pub config: Option<SdkFunctionConfig>,
    /// Function steps (for step functions)
    pub steps: Option<serde_json::Value>,
}

/// Trigger definition from SDK
#[derive(Debug, Deserialize)]
pub struct SdkTrigger {
    /// Event name
    pub event: Option<String>,
    /// Cron expression
    pub cron: Option<String>,
    /// Expression for conditional triggers
    pub expression: Option<String>,
}

/// Function configuration from SDK
#[derive(Debug, Deserialize)]
pub struct SdkFunctionConfig {
    /// Concurrency settings
    pub concurrency: Option<u32>,
    /// Batch configuration
    pub batch: Option<serde_json::Value>,
    /// Rate limiting
    pub rate_limit: Option<serde_json::Value>,
    /// Retry configuration
    pub retries: Option<u32>,
    /// Throttle configuration
    pub throttle: Option<serde_json::Value>,
    /// Priority
    pub priority: Option<u32>,
}

impl FunctionDiscovery {
    /// Create a new function discovery service
    pub fn new(router: Arc<RwLock<EventRouter>>, config: DiscoveryConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            router,
            config,
            function_cache: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start function discovery
    pub async fn start(&self) -> Result<()> {
        info!("Starting function discovery");

        // Initial discovery
        self.discover_all().await?;

        // Start polling if enabled
        if self.config.polling_enabled {
            let discovery_clone = self.clone();
            tokio::spawn(async move {
                discovery_clone.poll_loop().await;
            });
        }

        Ok(())
    }

    /// Discover functions from all configured URLs
    pub async fn discover_all(&self) -> Result<usize> {
        let mut total_discovered = 0;

        for url in &self.config.discovery_urls {
            match self.discover_from_url(url).await {
                Ok(count) => {
                    info!(url = %url, count = count, "Discovered functions");
                    total_discovered += count;
                }
                Err(e) => {
                    warn!(url = %url, error = %e, "Failed to discover functions");
                }
            }
        }

        info!(total = total_discovered, "Function discovery completed");
        Ok(total_discovered)
    }

    /// Discover functions from a specific URL
    pub async fn discover_from_url(&self, url: &str) -> Result<usize> {
        debug!(url = %url, "Discovering functions from URL");

        // Build request
        let mut request = self.client.get(url);

        // Add headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Add Inngest headers
        request = request
            .header("Content-Type", "application/json")
            .header("User-Agent", "inngest-rust/0.1.0")
            .header("X-Inngest-Framework", "rust")
            .header("X-Inngest-SDK", "rust-0.1.0");

        // Make request
        let response = request.send().await.map_err(|e| InngestError::Http {
            message: format!("Request failed: {}", e),
        })?;

        if !response.status().is_success() {
            return Err(InngestError::Http {
                message: format!(
                    "HTTP {}: {}",
                    response.status(),
                    response.status().canonical_reason().unwrap_or("Unknown")
                ),
            });
        }

        // Parse response
        let sdk_response: SdkResponse = response.json().await.map_err(|e| InngestError::Http {
            message: format!("Failed to parse response: {}", e),
        })?;

        // Convert SDK functions to internal format
        let mut discovered_functions = Vec::new();
        let app_id = Uuid::new_v4(); // TODO: Use actual app ID from config

        for sdk_func in sdk_response.functions {
            match self.convert_sdk_function(sdk_func, app_id, url).await {
                Ok(discovered_func) => discovered_functions.push(discovered_func),
                Err(e) => warn!(error = %e, "Failed to convert SDK function"),
            }
        }

        // Register functions with router
        let mut router = self.router.write().await;
        for discovered_func in &discovered_functions {
            if let Err(e) = router.register_function(discovered_func.function.clone()) {
                warn!(
                    function_id = %discovered_func.function.id,
                    error = %e,
                    "Failed to register function"
                );
            }
        }

        // Update cache
        let mut cache = self.function_cache.write().await;

        // Remove old functions from this URL
        cache.retain(|f| f.source_url != url);

        // Add new functions
        cache.extend(discovered_functions.clone());

        Ok(discovered_functions.len())
    }

    /// Convert SDK function to internal format
    async fn convert_sdk_function(
        &self,
        sdk_func: SdkFunction,
        app_id: Uuid,
        source_url: &str,
    ) -> Result<DiscoveredFunction> {
        // Convert triggers
        let triggers: Vec<Trigger> = sdk_func
            .triggers
            .into_iter()
            .filter_map(|t| {
                if let Some(event) = t.event {
                    Some(Trigger::Event {
                        event,
                        expression: t.expression,
                    })
                } else if let Some(cron) = t.cron {
                    Some(Trigger::Cron { cron })
                } else {
                    None
                }
            })
            .collect();

        // Convert configuration
        let config = FunctionConfig {
            id: sdk_func.id.clone(),
            name: sdk_func.name.clone(),
            triggers,
            concurrency: None, // TODO: Map SDK concurrency to our concurrency type
            rate_limit: None,  // TODO: Map SDK rate limit to our rate limit type
            batch: None,       // TODO: Map SDK batch config
            retry: None,       // TODO: Map SDK retry config
            steps: Vec::new(), // TODO: Parse steps from SDK
        };

        // Create function
        let function = Function {
            id: Uuid::new_v4(),
            config,
            version: 1,
            app_id,
            slug: sdk_func.id.clone(),
        };

        // Create metadata
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("sdk_id".to_string(), serde_json::Value::String(sdk_func.id));
        metadata.insert(
            "framework".to_string(),
            serde_json::Value::String("unknown".to_string()),
        );

        if let Some(steps) = sdk_func.steps {
            metadata.insert("steps".to_string(), steps);
        }

        Ok(DiscoveredFunction {
            function,
            source_url: source_url.to_string(),
            discovered_at: chrono::Utc::now(),
            active: true,
            metadata,
        })
    }

    /// Continuous polling loop
    async fn poll_loop(&self) {
        let mut interval = tokio::time::interval(self.config.poll_interval);

        loop {
            interval.tick().await;

            debug!("Running function discovery poll");

            if let Err(e) = self.discover_all().await {
                error!(error = %e, "Error during function discovery poll");
            }
        }
    }

    /// Get all discovered functions
    pub async fn get_functions(&self) -> Vec<DiscoveredFunction> {
        self.function_cache.read().await.clone()
    }

    /// Get functions from a specific URL
    pub async fn get_functions_from_url(&self, url: &str) -> Vec<DiscoveredFunction> {
        self.function_cache
            .read()
            .await
            .iter()
            .filter(|f| f.source_url == url)
            .cloned()
            .collect()
    }

    /// Clear discovered functions
    pub async fn clear(&self) -> Result<()> {
        info!("Clearing discovered functions");

        // Clear router
        let mut router = self.router.write().await;
        router.clear();

        // Clear cache
        let mut cache = self.function_cache.write().await;
        cache.clear();

        Ok(())
    }

    /// Get discovery statistics
    pub async fn get_stats(&self) -> DiscoveryStats {
        let cache = self.function_cache.read().await;
        let router = self.router.read().await;

        let total_functions = cache.len();
        let active_functions = cache.iter().filter(|f| f.active).count();
        let urls_count = self.config.discovery_urls.len();
        let router_stats = router.get_stats();

        DiscoveryStats {
            total_functions,
            active_functions,
            discovery_urls: urls_count,
            router_patterns: router_stats.get("total_patterns").copied().unwrap_or(0),
            last_discovery: cache.iter().map(|f| f.discovered_at).max(),
        }
    }
}

/// Function discovery statistics
#[derive(Debug, Serialize)]
pub struct DiscoveryStats {
    pub total_functions: usize,
    pub active_functions: usize,
    pub discovery_urls: usize,
    pub router_patterns: usize,
    pub last_discovery: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            discovery_urls: Vec::new(),
            timeout: Duration::from_secs(10),
            poll_interval: Duration::from_secs(30),
            polling_enabled: true,
            headers: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_config_defaults() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(10));
        assert_eq!(config.poll_interval, Duration::from_secs(30));
        assert!(config.polling_enabled);
        assert!(config.discovery_urls.is_empty());
    }

    #[tokio::test]
    async fn test_function_discovery_creation() {
        let router = Arc::new(RwLock::new(EventRouter::new()));
        let config = DiscoveryConfig::default();

        let discovery = FunctionDiscovery::new(router, config);
        let stats = discovery.get_stats().await;

        assert_eq!(stats.total_functions, 0);
        assert_eq!(stats.active_functions, 0);
    }
}
