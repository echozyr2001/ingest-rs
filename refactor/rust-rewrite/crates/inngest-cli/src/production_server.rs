use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::{serve, Router};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use uuid::Uuid;

use inngest_executor::QueueConsumer;
use inngest_queue::{redis_queue::RedisQueue, Producer};
use inngest_state::{postgres_state::PostgresStateManager, StateManager};

use crate::production_state::{PostgresProductionStateManager, ProductionStateManager};

/// Production server with API endpoints and queue consumer
pub struct ProductionServer {
    state_manager: Arc<PostgresProductionStateManager>,
    postgres_state: Arc<PostgresStateManager>,
    redis_queue: Arc<RedisQueue>,
    consumer_shutdown: Option<tokio::sync::watch::Sender<bool>>,
    consumer_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ProductionServer {
    pub async fn new(
        database_url: &str,
        redis_url: &str,
        function_base_url: String,
    ) -> Result<Self> {
        // Initialize PostgreSQL state manager
        let state_manager = Arc::new(
            PostgresProductionStateManager::from_url(database_url).await?
        );

        // Initialize regular PostgreSQL state for executor
        let postgres_state = Arc::new(
            PostgresStateManager::from_url(database_url).await?
        );

        // Initialize Redis queue
        let redis_queue = Arc::new(
            RedisQueue::new(redis_url, "inngest").await?
        );

        // Create queue consumer
        let (mut consumer, shutdown_tx) = QueueConsumer::new(
            Arc::clone(&redis_queue),
            Arc::clone(&postgres_state) as Arc<dyn StateManager>,
            function_base_url,
            5, // max concurrent executions
        ).await?;

        // Start consumer in background
        let consumer_handle = tokio::spawn(async move {
            if let Err(e) = consumer.start().await {
                tracing::error!("Queue consumer failed: {}", e);
            }
        });

        Ok(Self {
            state_manager,
            postgres_state,
            redis_queue,
            consumer_shutdown: Some(shutdown_tx),
            consumer_handle: Some(consumer_handle),
        })
    }

    /// Start the production server
    pub async fn start(&mut self, host: &str, port: u16) -> Result<()> {
        let app_state = ProductionServerState {
            state_manager: Arc::clone(&self.state_manager),
            redis_queue: Arc::clone(&self.redis_queue),
        };

        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/api/v1/functions", post(register_functions_handler))
            .route("/api/v1/functions", get(list_functions_handler))
            .route("/api/v1/events", post(submit_event_handler))
            .route("/api/v1/runs", get(list_runs_handler))
            .with_state(app_state)
            .layer(TraceLayer::new_for_http());

        let addr = format!("{}:{}", host, port);
        info!("🚀 Production server starting on {}", addr);
        info!("📊 Health endpoint: http://{}/health", addr);
        info!("🔧 Functions API: http://{}/api/v1/functions", addr);
        info!("📤 Events API: http://{}/api/v1/events", addr);
        info!("📋 Runs API: http://{}/api/v1/runs", addr);

        let listener = TcpListener::bind(&addr).await?;
        serve(listener, app).await?;

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down production server...");

        // Signal consumer to shutdown
        if let Some(shutdown_tx) = self.consumer_shutdown.take() {
            let _ = shutdown_tx.send(true);
        }

        // Wait for consumer to finish
        if let Some(handle) = self.consumer_handle.take() {
            info!("Waiting for queue consumer to shutdown...");
            let _ = handle.await;
        }

        info!("Production server shutdown complete");
        Ok(())
    }
}

#[derive(Clone)]
struct ProductionServerState {
    state_manager: Arc<PostgresProductionStateManager>,
    redis_queue: Arc<RedisQueue>,
}

/// Health check endpoint
async fn health_handler(
    State(state): State<ProductionServerState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Check PostgreSQL connection
    let postgres_status = match state.state_manager.health_check().await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    // Check Redis connection
    let redis_status = match state.redis_queue.health_check().await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    // Get memory usage
    let memory_info = get_memory_usage();

    Ok(Json(json!({
        "status": "ok",
        "postgres": postgres_status,
        "redis": redis_status,
        "memory_mb": memory_info.rss_mb,
        "consumer": {
            "status": "running",
            "max_concurrent": 5
        }
    })))
}

/// Register functions endpoint
async fn register_functions_handler(
    State(state): State<ProductionServerState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Parse the incoming function registration
    let function_data = payload.as_object().ok_or(StatusCode::BAD_REQUEST)?;
    
    let name = function_data
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let triggers = function_data
        .get("triggers")
        .and_then(|v| v.as_array())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Create function structure
    let function = inngest_core::Function {
        id: Uuid::new_v4(),
        config: inngest_core::FunctionConfig {
            id: format!("fn-{}", Uuid::new_v4()),
            name: name.to_string(),
            triggers: triggers.iter().filter_map(|t| {
                t.get("event").and_then(|e| e.as_str()).map(|event_name| {
                    inngest_core::Trigger::Event {
                        event: event_name.to_string(),
                        expression: None,
                    }
                })
            }).collect(),
            concurrency: None,
            rate_limit: None,
            batch: None,
            retry: None,
            steps: vec![],
        },
        version: 1,
        app_id: Uuid::new_v4(),
        slug: name.to_string(),
    };

    // Register the function
    match state.state_manager.register_function(&function).await {
        Ok(_) => {
            info!("Registered function: {}", name);
            Ok(Json(json!({
                "success": true,
                "function_id": function.config.id,
                "message": "Function registered successfully"
            })))
        }
        Err(e) => {
            warn!("Failed to register function {}: {}", name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List functions endpoint
async fn list_functions_handler(
    State(state): State<ProductionServerState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.state_manager.list_functions().await {
        Ok(functions) => {
            let function_list: Vec<serde_json::Value> = functions
                .iter()
                .map(|f| json!({
                    "id": f.config.id,
                    "name": f.config.name,
                    "triggers": f.config.triggers,
                    "version": f.version
                }))
                .collect();

            Ok(Json(json!({
                "functions": function_list,
                "count": function_list.len()
            })))
        }
        Err(e) => {
            warn!("Failed to list functions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Submit event endpoint
async fn submit_event_handler(
    State(state): State<ProductionServerState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let event_data = payload.as_object().ok_or(StatusCode::BAD_REQUEST)?;
    
    let event_name = event_data
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let event_payload = event_data
        .get("data")
        .cloned()
        .unwrap_or_else(|| json!({}));

    // Find functions that should handle this event
    let matching_functions = match state.state_manager.get_functions_for_event(event_name).await {
        Ok(functions) => functions,
        Err(e) => {
            warn!("Failed to find functions for event {}: {}", event_name, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut created_runs = Vec::new();

    // Create runs for each matching function
    for function in matching_functions {
        let run_id = Uuid::new_v4().to_string();
        
        // Create run record
        let run = inngest_core::Run {
            id: run_id.clone(),
            function_id: function.config.id.clone(),
            event_id: Uuid::new_v4().to_string(),
            status: "running".to_string(),
            started_at: Some(chrono::Utc::now()),
            ended_at: None,
            output: None,
            steps: vec![],
        };

        // Create run in database
        if let Err(e) = state.state_manager.create_run(&run).await {
            warn!("Failed to create run for function {}: {}", function.config.name, e);
            continue;
        }

        // Enqueue for execution
        let queue_item = inngest_queue::QueueItem {
            id: ulid::Ulid::new(),
            identifier: inngest_core::StateId::new(),
            edge: None,
            kind: inngest_queue::QueueItemKind::Start,
            attempt: 0,
            max_attempts: Some(3),
            priority: inngest_core::Priority::Normal,
            available_at: chrono::Utc::now(),
            group_id: None,
            payload: Some(json!({
                "function": function.config,
                "event": {
                    "name": event_name,
                    "data": event_payload
                },
                "run_id": run_id
            })),
            run_info: None,
        };

        if let Err(e) = state.redis_queue.enqueue(queue_item).await {
            warn!("Failed to enqueue execution for function {}: {}", function.config.name, e);
            continue;
        }

        created_runs.push(json!({
            "function_name": function.config.name,
            "run_id": run_id,
            "status": "queued"
        }));

        info!("Created run {} for function {} triggered by event {}", 
              run_id, function.config.name, event_name);
    }

    Ok(Json(json!({
        "success": true,
        "event_name": event_name,
        "runs_created": created_runs.len(),
        "runs": created_runs
    })))
}

/// List runs endpoint
async fn list_runs_handler(
    State(state): State<ProductionServerState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.state_manager.list_runs(50, 0, None, None).await {
        Ok(runs) => {
            let run_list: Vec<serde_json::Value> = runs
                .iter()
                .map(|r| json!({
                    "id": r.id,
                    "function_id": r.function_id,
                    "status": r.status,
                    "started_at": r.started_at,
                    "ended_at": r.ended_at,
                    "output": r.output
                }))
                .collect();

            Ok(Json(json!({
                "runs": run_list,
                "count": run_list.len()
            })))
        }
        Err(e) => {
            warn!("Failed to list runs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get memory usage information
fn get_memory_usage() -> MemoryInfo {
    use sysinfo::System;
    
    let mut system = System::new_all();
    system.refresh_all();
    
    let rss_bytes = if let Some(process) = system.process(sysinfo::get_current_pid().unwrap()) {
        process.memory() * 1024 // sysinfo returns KB, convert to bytes
    } else {
        0
    };

    MemoryInfo {
        rss_mb: (rss_bytes as f64 / 1024.0 / 1024.0).round() as u64,
    }
}

#[derive(Debug)]
struct MemoryInfo {
    rss_mb: u64,
}
