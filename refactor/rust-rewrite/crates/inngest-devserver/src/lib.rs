pub mod discovery;

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::{net::IpAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use uuid;

use inngest_core::{Event, EventRouter, Executor, Function, HttpExecutor};
use inngest_queue::memory_queue::MemoryQueue;
use inngest_state::memory_state::MemoryStateManager;

/// Configuration for the development server
#[derive(Debug, Clone)]
pub struct DevServerConfig {
    pub host: IpAddr,
    pub port: u16,
    pub urls: Vec<String>,
    pub discovery_enabled: bool,
    pub polling_enabled: bool,
    pub poll_interval_ms: u64,
}

/// Development server state
#[derive(Clone)]
pub struct DevServerState {
    pub config: DevServerConfig,
    pub functions: Arc<std::sync::RwLock<Vec<Function>>>,
    pub router: Arc<std::sync::RwLock<EventRouter>>,
    pub queue: Arc<MemoryQueue>,
    pub executor: Arc<HttpExecutor>,
    pub state_manager: Arc<MemoryStateManager>,
}

/// Development server
pub struct DevServer {
    state: DevServerState,
}

impl DevServer {
    pub async fn new(config: DevServerConfig) -> Result<Self> {
        let state_manager = Arc::new(MemoryStateManager::new());
        let queue = Arc::new(MemoryQueue::new());
        let executor = Arc::new(HttpExecutor::new(
            std::time::Duration::from_secs(30),
            "http://localhost:3000".to_string(), // Default base URL
        ));
        let router = Arc::new(std::sync::RwLock::new(EventRouter::new()));

        let state = DevServerState {
            config,
            functions: Arc::new(std::sync::RwLock::new(Vec::new())),
            router,
            queue,
            executor,
            state_manager,
        };

        Ok(Self { state })
    }

    pub async fn start(self) -> Result<()> {
        let app = self.create_router();

        let addr = format!("{}:{}", self.state.config.host, self.state.config.port);
        let listener = TcpListener::bind(&addr).await?;

        info!("Inngest dev server listening on {}", addr);
        info!("Dashboard available at http://{}", addr);

        axum::serve(listener, app).await?;
        Ok(())
    }

    fn create_router(&self) -> Router {
        Router::new()
            .route("/", get(dashboard_handler))
            .route("/health", get(health_handler))
            .route("/e/:event_key", post(event_handler))
            .route("/fn/:function_id", post(function_handler))
            .route("/api/v1/events", post(api_events_handler))
            .route("/api/v1/functions", get(api_functions_handler))
            .route("/api/v1/functions", post(register_function_handler))
            .route("/api/v1/runs", get(api_runs_handler))
            .layer(CorsLayer::permissive())
            .with_state(self.state.clone())
    }
}

// Handler functions

async fn dashboard_handler() -> &'static str {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Inngest Dev Server</title>
        <style>
            body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
            h1 { color: #6366f1; }
            .status { background: #f0f9ff; padding: 10px; border-radius: 5px; }
        </style>
    </head>
    <body>
        <h1>🎯 Inngest Dev Server</h1>
        <div class="status">
            <h2>Status: Running</h2>
            <p>The Inngest development server is running and ready to receive events and execute functions.</p>
        </div>
        <h2>Endpoints</h2>
        <ul>
            <li><code>POST /e/{event_key}</code> - Send events</li>
            <li><code>GET /api/v1/functions</code> - List functions</li>
            <li><code>GET /api/v1/runs</code> - List function runs</li>
        </ul>
    </body>
    </html>
    "#
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "inngest-dev-server",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[derive(Deserialize)]
struct EventRequest {
    name: String,
    data: serde_json::Value,
    user: Option<serde_json::Value>,
    version: Option<String>,
}

async fn event_handler(
    State(state): State<DevServerState>,
    Json(req): Json<EventRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Received event: {}", req.name);

    let event = Event {
        id: Some(uuid::Uuid::new_v4()),
        name: req.name.clone(),
        data: req.data,
        user: req.user,
        version: req.version,
        ts: Some(chrono::Utc::now()),
    };

    // Route event to matching functions
    let routing_result = {
        let router = state.router.read().unwrap();
        match router.route_event(&event) {
            Ok(result) => result,
            Err(e) => {
                warn!("Failed to route event {}: {}", req.name, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    };

    info!(
        "Event {} matched {} functions",
        req.name,
        routing_result.matched_functions.len()
    );

    // Execute matched functions (simplified for dev server)
    let mut execution_results = Vec::new();
    for function in &routing_result.matched_functions {
        info!(
            "Executing function: {} ({})",
            function.config.name, function.id
        );

        // Create execution context
        let context = inngest_core::ExecutionContext {
            run_id: uuid::Uuid::new_v4(),
            function_id: function.id.to_string(),
            event: event.clone(),
            attempt: 1,
            metadata: std::collections::HashMap::new(),
        };

        // Execute function (in background for dev server)
        match state.executor.execute(function, context).await {
            Ok(result) => {
                info!(
                    "Function {} executed successfully: success={}",
                    function.config.name, result.success
                );
                execution_results.push(serde_json::json!({
                    "function_id": function.id,
                    "function_name": function.config.name,
                    "success": result.success,
                    "output": result.output
                }));
            }
            Err(e) => {
                warn!("Function {} execution failed: {}", function.config.name, e);
                execution_results.push(serde_json::json!({
                    "function_id": function.id,
                    "function_name": function.config.name,
                    "success": false,
                    "error": e.to_string()
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "event": req.name,
        "event_id": event.id,
        "matched_functions": routing_result.matched_functions.len(),
        "executions": execution_results,
        "routing_metadata": routing_result.routing_metadata
    })))
}

async fn function_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Function endpoint - to be implemented"
    }))
}

async fn api_events_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "events": []
    }))
}

async fn api_functions_handler(State(state): State<DevServerState>) -> Json<serde_json::Value> {
    let functions = state.functions.read().unwrap();
    let function_list: Vec<_> = functions
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "name": f.config.name,
                "slug": f.slug,
                "triggers": f.config.triggers,
                "version": f.version
            })
        })
        .collect();

    Json(serde_json::json!({
        "functions": function_list
    }))
}

async fn api_runs_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "runs": []
    }))
}

#[derive(Deserialize)]
struct RegisterFunctionRequest {
    name: String,
    triggers: Vec<serde_json::Value>,
    slug: Option<String>,
}

async fn register_function_handler(
    State(state): State<DevServerState>,
    payload: axum::extract::Request,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Parse JSON manually to provide better error handling
    let body = match axum::body::to_bytes(payload.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Failed to read request body",
                    "success": false
                })),
            ));
        }
    };

    let req: RegisterFunctionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid JSON: {}", e),
                    "success": false
                })),
            ));
        }
    };

    info!("Registering function: {}", req.name);

    // Convert triggers (simplified for testing)
    let mut triggers = Vec::new();
    for trigger_data in req.triggers {
        if let Some(event_name) = trigger_data.get("event").and_then(|v| v.as_str()) {
            triggers.push(inngest_core::function::Trigger::Event {
                event: event_name.to_string(),
                expression: None,
            });
        }
    }

    // Create function
    let function = Function {
        id: uuid::Uuid::new_v4(),
        config: inngest_core::function::FunctionConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.name.clone(),
            triggers,
            concurrency: None,
            rate_limit: None,
            batch: None,
            retry: None,
            steps: Vec::new(),
        },
        version: 1,
        app_id: uuid::Uuid::new_v4(),
        slug: req
            .slug
            .unwrap_or_else(|| req.name.replace(' ', "-").to_lowercase()),
    };

    // Register function in state
    {
        let mut functions = state.functions.write().unwrap();
        functions.push(function.clone());
    }

    // Register function in router
    {
        let mut router = state.router.write().unwrap();
        if let Err(e) = router.register_function(function.clone()) {
            warn!("Failed to register function in router: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to register function: {}", e),
                    "success": false
                })),
            ));
        }
    }

    info!("Function {} registered successfully", req.name);

    Ok(Json(serde_json::json!({
        "success": true,
        "function": {
            "id": function.id,
            "name": function.config.name,
            "slug": function.slug,
            "triggers": function.config.triggers,
        }
    })))
}
