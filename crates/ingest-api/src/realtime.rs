//! Real-time API endpoints
//!
//! This module provides REST and WebSocket endpoints for real-time features
//! including status updates, event streaming, and live monitoring.

use crate::error::Result;
use crate::types::ApiResponse;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    Json, Router,
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
    routing::get,
};

use ingest_connect::{
    ConnectionId, EventStreamer, LiveMonitor, SessionId, StatusBroadcaster, StatusFilter,
    StreamFilter, monitoring::DashboardSubscription,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Real-time API state
#[derive(Clone)]
pub struct RealtimeState {
    pub status_broadcaster: Arc<StatusBroadcaster>,
    pub event_streamer: Arc<EventStreamer>,
    pub live_monitor: Arc<LiveMonitor>,
}

/// Status subscription request
#[derive(Debug, Deserialize)]
pub struct StatusSubscriptionRequest {
    pub filter: Option<StatusFilter>,
}

/// Stream subscription request
#[derive(Debug, Deserialize)]
pub struct StreamSubscriptionRequest {
    pub filter: Option<StreamFilter>,
}

/// Dashboard subscription request
#[derive(Debug, Deserialize)]
pub struct DashboardSubscriptionRequest {
    pub subscription: Option<DashboardSubscription>,
}

/// Real-time statistics response
#[derive(Debug, Serialize)]
pub struct RealtimeStats {
    pub status_subscribers: usize,
    pub event_streams: usize,
    pub dashboard_connections: usize,
    pub uptime_seconds: u64,
}

/// Create real-time routes
pub fn create_realtime_routes() -> Router<RealtimeState> {
    Router::new()
        .route("/status", get(get_realtime_stats))
        .route("/status/subscribe", get(subscribe_status_updates))
        .route("/events/stream", get(stream_events))
        .route("/dashboard", get(dashboard_websocket))
        .route("/health", get(realtime_health_check))
}

/// Get real-time system statistics
async fn get_realtime_stats(
    State(state): State<RealtimeState>,
) -> Result<Json<ApiResponse<RealtimeStats>>> {
    let stats = RealtimeStats {
        status_subscribers: state.status_broadcaster.subscription_count().await,
        event_streams: state.event_streamer.connection_count().await,
        dashboard_connections: state.live_monitor.connection_count().await,
        uptime_seconds: 0, // TODO: Implement uptime tracking
    };

    Ok(Json(ApiResponse::new(stats)))
}

/// WebSocket endpoint for status updates
async fn subscribe_status_updates(
    ws: WebSocketUpgrade,
    Query(params): Query<StatusSubscriptionRequest>,
    State(state): State<RealtimeState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_status_websocket(socket, params, state))
}

/// WebSocket endpoint for event streaming
async fn stream_events(
    ws: WebSocketUpgrade,
    Query(params): Query<StreamSubscriptionRequest>,
    State(state): State<RealtimeState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_stream_websocket(socket, params, state))
}

/// WebSocket endpoint for dashboard
async fn dashboard_websocket(
    ws: WebSocketUpgrade,
    Query(params): Query<DashboardSubscriptionRequest>,
    State(state): State<RealtimeState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_dashboard_websocket(socket, params, state))
}

/// Health check for real-time services
async fn realtime_health_check(
    State(state): State<RealtimeState>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let health_data = serde_json::json!({
        "status": "healthy",
        "services": {
            "status_broadcaster": "running",
            "event_streamer": "running",
            "live_monitor": "running"
        },
        "connections": {
            "status_subscribers": state.status_broadcaster.subscription_count().await,
            "event_streams": state.event_streamer.connection_count().await,
            "dashboard_connections": state.live_monitor.connection_count().await
        }
    });

    Ok(Json(ApiResponse::new(health_data)))
}

/// Handle status update WebSocket connection
async fn handle_status_websocket(
    mut socket: WebSocket,
    params: StatusSubscriptionRequest,
    state: RealtimeState,
) {
    let connection_id = ConnectionId::new();
    let session_id = SessionId::new();
    let filter = params.filter.unwrap_or_default();

    // Subscribe to status updates
    let mut receiver = state
        .status_broadcaster
        .subscribe(connection_id, session_id, filter)
        .await;

    // Handle WebSocket messages
    loop {
        tokio::select! {
            // Receive status updates
            status_update = receiver.recv() => {
                match status_update {
                    Ok(update) => {
                        let message = serde_json::to_string(&update).unwrap();
                        if socket.send(Message::Text(message.into())).await.is_err() {
                            break;
                        }

                        // Update activity
                        state.status_broadcaster.update_activity(connection_id).await;
                    }
                    Err(_) => break, // Channel closed
                }
            }

            // Receive WebSocket messages
            ws_message = socket.recv() => {
                match ws_message {
                    Some(Ok(Message::Text(text))) => {
                        // Handle client messages (e.g., filter updates)
                        if text == "ping"
                            && socket.send(Message::Text("pong".to_string().into())).await.is_err() {
                                break;
                            }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // Ignore other message types
                }
            }
        }
    }

    // Cleanup subscription
    state.status_broadcaster.unsubscribe(connection_id).await;
}

/// Handle event stream WebSocket connection
async fn handle_stream_websocket(
    mut socket: WebSocket,
    params: StreamSubscriptionRequest,
    state: RealtimeState,
) {
    let connection_id = ConnectionId::new();
    let session_id = SessionId::new();
    let filter = params.filter.unwrap_or_default();

    // Create event stream
    let mut receiver = match state
        .event_streamer
        .create_stream(connection_id, session_id, filter)
        .await
    {
        Ok(receiver) => receiver,
        Err(_) => {
            let _ = socket
                .send(Message::Text(
                    "Error: Failed to create stream".to_string().into(),
                ))
                .await;
            return;
        }
    };

    // Handle WebSocket messages
    loop {
        tokio::select! {
            // Receive stream events
            stream_event = receiver.recv() => {
                match stream_event {
                    Ok(event) => {
                        let message = serde_json::to_string(&event).unwrap();
                        if socket.send(Message::Text(message.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }

            // Receive WebSocket messages
            ws_message = socket.recv() => {
                match ws_message {
                    Some(Ok(Message::Text(text))) => {
                        // Handle client messages
                        if text == "ping"
                            && socket.send(Message::Text("pong".to_string().into())).await.is_err() {
                                break;
                            }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // Ignore other message types
                }
            }
        }
    }

    // Cleanup stream
    state.event_streamer.remove_stream(connection_id).await;
}

/// Handle dashboard WebSocket connection
async fn handle_dashboard_websocket(
    mut socket: WebSocket,
    params: DashboardSubscriptionRequest,
    state: RealtimeState,
) {
    let connection_id = ConnectionId::new();
    let session_id = SessionId::new();
    let subscription = params.subscription.unwrap_or_default();

    // Subscribe to dashboard updates
    let mut receiver = state
        .live_monitor
        .subscribe_dashboard(connection_id, session_id, subscription)
        .await;

    // Handle WebSocket messages
    loop {
        tokio::select! {
            // Receive dashboard updates
            dashboard_update = receiver.recv() => {
                match dashboard_update {
                    Ok(update) => {
                        let message = serde_json::to_string(&update).unwrap();
                        if socket.send(Message::Text(message.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break, // Channel closed
                }
            }

            // Receive WebSocket messages
            ws_message = socket.recv() => {
                match ws_message {
                    Some(Ok(Message::Text(text))) => {
                        // Handle client messages
                        if text == "ping"
                            && socket.send(Message::Text("pong".to_string().into())).await.is_err() {
                                break;
                            }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // Ignore other message types
                }
            }
        }
    }

    // Cleanup subscription
    state
        .live_monitor
        .unsubscribe_dashboard(connection_id)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_connect::StreamConfig;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_realtime_stats_creation() {
        let stats = RealtimeStats {
            status_subscribers: 10,
            event_streams: 5,
            dashboard_connections: 3,
            uptime_seconds: 3600,
        };

        assert_eq!(stats.status_subscribers, 10);
        assert_eq!(stats.event_streams, 5);
        assert_eq!(stats.dashboard_connections, 3);
        assert_eq!(stats.uptime_seconds, 3600);
    }

    #[test]
    fn test_status_subscription_request_default() {
        let request = StatusSubscriptionRequest { filter: None };
        assert!(request.filter.is_none());
    }

    #[test]
    fn test_stream_subscription_request_default() {
        let request = StreamSubscriptionRequest { filter: None };
        assert!(request.filter.is_none());
    }

    #[test]
    fn test_dashboard_subscription_request_default() {
        let request = DashboardSubscriptionRequest { subscription: None };
        assert!(request.subscription.is_none());
    }

    #[tokio::test]
    async fn test_realtime_state_creation() {
        use std::time::Duration;

        let status_broadcaster = Arc::new(StatusBroadcaster::new(1000, Duration::from_secs(300)));
        let event_streamer = Arc::new(EventStreamer::new(StreamConfig::default()));
        let live_monitor = Arc::new(LiveMonitor::new(
            1000,
            Duration::from_secs(1),
            Duration::from_secs(60),
        ));

        let state = RealtimeState {
            status_broadcaster,
            event_streamer,
            live_monitor,
        };

        // Test that state can be cloned
        let _cloned_state = state.clone();
    }
}
