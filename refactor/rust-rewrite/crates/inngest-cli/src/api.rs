use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use inngest_core::{event::Event, function::Function, run::Run};
use inngest_queue::RedisQueue;

use crate::production_state::{PostgresProductionStateManager, ProductionStateManager};

/// Production API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub state_manager: PostgresProductionStateManager,
    pub queue: RedisQueue,
}

/// Function registration request
#[derive(Debug, Deserialize)]
pub struct FunctionRegistrationRequest {
    pub functions: Vec<Function>,
}

/// Function registration response
#[derive(Debug, Serialize)]
pub struct FunctionRegistrationResponse {
    pub status: String,
    pub registered: usize,
    pub modified: usize,
    pub skipped: usize,
}

/// Event submission request
#[derive(Debug, Deserialize)]
pub struct EventRequest {
    pub name: String,
    pub data: Value,
    pub user: Option<Value>,
    pub id: Option<String>,
    pub ts: Option<i64>,
}

/// Event submission response
#[derive(Debug, Serialize)]
pub struct EventResponse {
    pub id: String,
    pub status: String,
}

/// Query parameters for runs listing
#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub status: Option<String>,
    pub function_id: Option<String>,
}

/// Create production API router
pub fn create_api_router() -> Router<ApiState> {
    Router::new()
        // Function management endpoints
        .route("/api/v1/functions", get(list_functions))
        .route("/api/v1/functions", post(register_functions))
        // Event processing endpoints
        .route("/api/v1/events", post(submit_event))
        // Run management endpoints
        .route("/api/v1/runs", get(list_runs))
        .route("/api/v1/runs/:run_id", get(get_run))
        // Health check endpoints
        .route("/health", get(health_check))
        .route("/", get(root_handler))
}

/// List all registered functions
pub async fn list_functions(State(state): State<ApiState>) -> Result<Json<Value>, StatusCode> {
    tracing::debug!("Listing all functions");

    match state.state_manager.list_functions().await {
        Ok(functions) => {
            tracing::info!("Successfully listed {} functions", functions.len());
            Ok(Json(json!({
                "functions": functions,
                "count": functions.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to list functions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Register new functions
pub async fn register_functions(
    State(state): State<ApiState>,
    Json(payload): Json<FunctionRegistrationRequest>,
) -> Result<Json<FunctionRegistrationResponse>, StatusCode> {
    tracing::debug!("Registering {} functions", payload.functions.len());

    let mut registered = 0;
    let mut modified = 0;
    let mut skipped = 0;

    for function in payload.functions {
        match state.state_manager.register_function(&function).await {
            Ok(true) => {
                registered += 1;
                tracing::debug!("Registered new function: {}", function.config.id);
            }
            Ok(false) => {
                modified += 1;
                tracing::debug!("Modified existing function: {}", function.config.id);
            }
            Err(e) => {
                skipped += 1;
                tracing::warn!("Failed to register function {}: {}", function.config.id, e);
            }
        }
    }

    tracing::info!(
        "Function registration complete: {} registered, {} modified, {} skipped",
        registered,
        modified,
        skipped
    );

    Ok(Json(FunctionRegistrationResponse {
        status: "ok".to_string(),
        registered,
        modified,
        skipped,
    }))
}

/// Submit a new event for processing
pub async fn submit_event(
    State(state): State<ApiState>,
    Json(payload): Json<EventRequest>,
) -> Result<Json<EventResponse>, StatusCode> {
    let event_id = payload.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    tracing::debug!("Processing event: {} ({})", payload.name, event_id);

    // Create event
    let event = Event {
        id: Some(Uuid::parse_str(&event_id).unwrap_or_else(|_| Uuid::new_v4())),
        name: payload.name,
        data: payload.data,
        user: payload.user,
        version: Some("2024-01-01".to_string()),
        ts: payload
            .ts
            .map(|ts| DateTime::from_timestamp_millis(ts).unwrap_or_else(|| Utc::now())),
    };

    // Get matching functions
    match state
        .state_manager
        .get_functions_for_event(&event.name)
        .await
    {
        Ok(functions) => {
            if functions.is_empty() {
                tracing::info!("No functions found for event: {}", event.name);
                return Ok(Json(EventResponse {
                    id: event_id,
                    status: "no_functions".to_string(),
                }));
            }

            // Queue runs for each matching function
            let mut queued_runs = 0;
            let functions_count = functions.len();
            for function in &functions {
                let run_id = Uuid::new_v4().to_string();

                let run = Run {
                    id: run_id.clone(),
                    function_id: function.config.id.clone(),
                    event_id: event_id.clone(),
                    status: "queued".to_string(),
                    started_at: None,
                    ended_at: None,
                    output: None,
                    steps: vec![],
                };

                // Save run to database
                if let Err(e) = state.state_manager.create_run(&run).await {
                    tracing::error!("Failed to create run {}: {}", run_id, e);
                    continue;
                }

                // Queue run for execution
                if let Err(e) = state
                    .queue
                    .push(&run_id, &serde_json::to_string(&run).unwrap())
                    .await
                {
                    tracing::error!("Failed to queue run {}: {}", run_id, e);
                    continue;
                }

                queued_runs += 1;
                tracing::debug!("Queued run {} for function {}", run_id, function.config.id);
            }

            tracing::info!(
                "Event {} processed: {} runs queued for {} functions",
                event_id,
                queued_runs,
                functions_count
            );

            Ok(Json(EventResponse {
                id: event_id,
                status: "queued".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to get functions for event {}: {}", event.name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List function runs with optional filtering
pub async fn list_runs(
    State(state): State<ApiState>,
    Query(params): Query<RunsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(50).min(1000); // Cap at 1000
    let offset = params.offset.unwrap_or(0);

    tracing::debug!(
        "Listing runs: limit={}, offset={}, status={:?}, function_id={:?}",
        limit,
        offset,
        params.status,
        params.function_id
    );

    match state
        .state_manager
        .list_runs(limit, offset, params.status, params.function_id)
        .await
    {
        Ok(runs) => {
            tracing::info!("Successfully listed {} runs", runs.len());
            Ok(Json(json!({
                "runs": runs,
                "count": runs.len(),
                "limit": limit,
                "offset": offset
            })))
        }
        Err(e) => {
            tracing::error!("Failed to list runs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get a specific run by ID
pub async fn get_run(
    State(state): State<ApiState>,
    axum::extract::Path(run_id): axum::extract::Path<String>,
) -> Result<Json<Value>, StatusCode> {
    tracing::debug!("Getting run: {}", run_id);

    match state.state_manager.get_run(&run_id).await {
        Ok(Some(run)) => {
            tracing::info!("Successfully retrieved run: {}", run_id);
            Ok(Json(json!(run)))
        }
        Ok(None) => {
            tracing::warn!("Run not found: {}", run_id);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            tracing::error!("Failed to get run {}: {}", run_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Health check endpoint
pub async fn health_check(State(state): State<ApiState>) -> Json<Value> {
    // Test database connection
    let db_status = match state.state_manager.health_check().await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    // Test Redis connection
    let redis_status = match state.queue.ping().await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    Json(json!({
        "status": "ok",
        "service": "inngest-production-server",
        "version": env!("CARGO_PKG_VERSION"),
        "backends": {
            "postgres": db_status,
            "redis": redis_status
        }
    }))
}

/// Root handler
pub async fn root_handler() -> Json<Value> {
    Json(json!({
        "message": "Inngest Production Server API",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": {
            "functions": "/api/v1/functions",
            "events": "/api/v1/events",
            "runs": "/api/v1/runs",
            "health": "/health"
        }
    }))
}
