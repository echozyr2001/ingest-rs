//! WebSocket server for handling SDK connections

use crate::{
    auth::{AuthService, SdkCredentials},
    config::ConnectConfig,
    connection::{Connection, ConnectionState},
    error::{ConnectError, Result},
    manager::ConnectionManager,
    protocol::{AuthMethod, Message, ProtocolNegotiator, ProtocolVersion, ServerCapabilities},
    types::{ClientInfo, ConnectionId},
};
use axum::{
    Router,
    extract::{
        ConnectInfo, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_tungstenite::WebSocketStream;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{debug, error, info, warn};

/// WebSocket server for handling SDK connections
#[derive(Clone)]
pub struct ConnectServer {
    /// Server configuration
    config: Arc<ConnectConfig>,

    /// Connection manager
    manager: ConnectionManager,

    /// Authentication service
    auth_service: AuthService,

    /// Protocol negotiator
    protocol_negotiator: ProtocolNegotiator,
}

impl ConnectServer {
    /// Create a new connect server
    pub fn new(
        config: ConnectConfig,
        auth_service: AuthService,
        protocol_negotiator: ProtocolNegotiator,
    ) -> Self {
        let manager = ConnectionManager::new(config.clone(), auth_service.clone());

        Self {
            config: Arc::new(config),
            manager,
            auth_service,
            protocol_negotiator,
        }
    }

    /// Start the WebSocket server
    pub async fn start(&self) -> Result<()> {
        let bind_addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| ConnectError::Network(format!("Failed to bind to {bind_addr}: {e}")))?;

        info!(bind_addr = %bind_addr, "WebSocket server starting");

        // Start background tasks
        self.manager.start_background_tasks().await?;

        // Create Axum router for HTTP upgrade
        let app = self.create_router();

        // Start server
        let server = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        );

        info!(bind_addr = %bind_addr, "WebSocket server started");

        if let Err(e) = server.await {
            error!(error = %e, "Server error");
            return Err(ConnectError::Network(format!("Server error: {e}")));
        }

        Ok(())
    }

    /// Create Axum router
    fn create_router(&self) -> Router {
        Router::new()
            .route("/ws", get(Self::websocket_handler))
            .route("/health", get(Self::health_handler))
            .with_state(self.clone())
            .layer(
                ServiceBuilder::new()
                    .layer(
                        TraceLayer::new_for_http()
                            .make_span_with(DefaultMakeSpan::default().include_headers(true)),
                    )
                    .layer(CorsLayer::permissive()),
            )
    }

    /// WebSocket upgrade handler
    async fn websocket_handler(
        ws: WebSocketUpgrade,
        State(server): State<ConnectServer>,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        headers: HeaderMap,
    ) -> std::result::Result<Response, StatusCode> {
        info!(remote_addr = %addr, "WebSocket upgrade request");

        // Extract credentials from headers if available
        let credentials = server
            .auth_service
            .extract_credentials_from_headers(&headers)
            .map_err(|e| {
                warn!(error = %e, "Failed to extract credentials from headers");
                StatusCode::UNAUTHORIZED
            })?;

        // Upgrade to WebSocket
        Ok(ws.on_upgrade(move |socket| {
            server.handle_websocket_connection(socket, addr, credentials)
        }))
    }

    /// Health check handler
    async fn health_handler(
        State(server): State<ConnectServer>,
    ) -> std::result::Result<String, StatusCode> {
        let stats = server.manager.get_stats();
        Ok(format!(
            "OK - Active connections: {}, Total connections: {}",
            stats.active_connections, stats.total_connections
        ))
    }

    /// Handle a WebSocket connection
    async fn handle_websocket_connection(
        self,
        socket: WebSocket,
        remote_addr: SocketAddr,
        credentials: Option<SdkCredentials>,
    ) {
        let connection_id = ConnectionId::new();
        info!(
            connection_id = %connection_id,
            remote_addr = %remote_addr,
            "New WebSocket connection"
        );

        // Convert Axum WebSocket to tokio-tungstenite
        let ws_stream = match self.convert_websocket(socket).await {
            Ok(stream) => stream,
            Err(e) => {
                error!(
                    connection_id = %connection_id,
                    error = %e,
                    "Failed to convert WebSocket"
                );
                return;
            }
        };

        // Create connection
        let (connection, handle) =
            Connection::new(connection_id, remote_addr.to_string(), ws_stream);

        // Add to manager
        if let Err(e) = self.manager.add_connection(connection.clone()).await {
            error!(
                connection_id = %connection_id,
                error = %e,
                "Failed to add connection to manager"
            );
            return;
        }

        // Handle connection lifecycle
        let server = self.clone();
        let connection_clone = connection.clone();

        // Spawn connection handler
        let handler_task = tokio::spawn(async move { handle.run().await });

        // Spawn connection processor
        let processor_task = tokio::spawn(async move {
            server
                .process_connection(connection_clone, credentials)
                .await
        });

        // Wait for either task to complete
        tokio::select! {
            result = handler_task => {
                match result {
                    Ok(Ok(())) => debug!(connection_id = %connection_id, "Connection handler completed"),
                    Ok(Err(e)) => error!(connection_id = %connection_id, error = %e, "Connection handler failed"),
                    Err(e) => error!(connection_id = %connection_id, error = %e, "Connection handler panicked"),
                }
            }
            result = processor_task => {
                match result {
                    Ok(Ok(())) => debug!(connection_id = %connection_id, "Connection processor completed"),
                    Ok(Err(e)) => error!(connection_id = %connection_id, error = %e, "Connection processor failed"),
                    Err(e) => error!(connection_id = %connection_id, error = %e, "Connection processor panicked"),
                }
            }
        }

        // Clean up
        if let Err(e) = self.manager.remove_connection(connection_id).await {
            error!(
                connection_id = %connection_id,
                error = %e,
                "Failed to remove connection from manager"
            );
        }

        info!(connection_id = %connection_id, "Connection closed");
    }

    /// Process connection messages and handle protocol
    async fn process_connection(
        &self,
        connection: Connection,
        initial_credentials: Option<SdkCredentials>,
    ) -> Result<()> {
        let connection_id = connection.id().await;

        // Set initial state
        connection.set_state(ConnectionState::Handshaking).await;

        // Handle handshake
        let (protocol_version, client_info) = self.handle_handshake(&connection).await?;

        // Set protocol version
        connection
            .set_protocol_version(protocol_version.clone())
            .await;
        connection.set_client_info(client_info.clone()).await;

        // Handle authentication
        let credentials = if let Some(creds) = initial_credentials {
            creds
        } else {
            self.handle_authentication(&connection).await?
        };

        // Authenticate with manager
        self.manager
            .authenticate_connection(connection_id, &credentials, client_info)
            .await?;

        info!(
            connection_id = %connection_id,
            sdk_id = %credentials.sdk_id,
            protocol_version = %protocol_version,
            "Connection authenticated and ready"
        );

        // Main message loop
        self.handle_messages(&connection).await?;

        Ok(())
    }

    /// Handle protocol handshake
    async fn handle_handshake(
        &self,
        connection: &Connection,
    ) -> Result<(ProtocolVersion, ClientInfo)> {
        let connection_id = connection.id().await;

        // Wait for handshake message
        let handshake_message = timeout(
            self.config.protocol.negotiation_timeout,
            connection.receive_message(),
        )
        .await
        .map_err(|_| ConnectError::Timeout("Handshake timeout".to_string()))?
        .map_err(|e| ConnectError::ProtocolNegotiation(e.to_string()))?
        .ok_or_else(|| {
            ConnectError::ProtocolNegotiation("No handshake message received".to_string())
        })?;

        match handshake_message {
            Message::Handshake {
                versions,
                client_info,
            } => {
                debug!(
                    connection_id = %connection_id,
                    client_versions = ?versions,
                    "Received handshake"
                );

                // Negotiate protocol version
                let negotiated_version = self.protocol_negotiator.negotiate(&versions)?;

                // Send handshake response
                let response = Message::HandshakeResponse {
                    version: negotiated_version.clone(),
                    session_id: connection.session_id().await,
                    capabilities: ServerCapabilities {
                        features: vec![
                            "events".to_string(),
                            "functions".to_string(),
                            "webhooks".to_string(),
                        ],
                        max_message_size: self.config.max_message_size,
                        heartbeat_interval: self.config.heartbeat_interval.as_secs(),
                        compression: self.config.protocol.compression.enabled,
                    },
                };

                connection.send_message(response).await?;

                Ok((negotiated_version, client_info))
            }
            _ => Err(ConnectError::ProtocolNegotiation(
                "Expected handshake message".to_string(),
            )),
        }
    }

    /// Handle authentication
    async fn handle_authentication(&self, connection: &Connection) -> Result<SdkCredentials> {
        let connection_id = connection.id().await;

        connection.set_state(ConnectionState::Authenticating).await;

        // Wait for authentication message
        let auth_message = timeout(
            Duration::from_secs(30), // Auth timeout
            connection.receive_message(),
        )
        .await
        .map_err(|_| ConnectError::Timeout("Authentication timeout".to_string()))?
        .map_err(|e| ConnectError::Authentication(e.to_string()))?
        .ok_or_else(|| {
            ConnectError::Authentication("No authentication message received".to_string())
        })?;

        match auth_message {
            Message::Auth { token, method } => {
                debug!(
                    connection_id = %connection_id,
                    auth_method = ?method,
                    "Received authentication"
                );

                // Create credentials based on auth method
                let credentials = match method {
                    AuthMethod::Jwt => {
                        // Validate token to get SDK info
                        let claims = self.auth_service.validate_jwt_token(&token)?;
                        SdkCredentials::new(claims.sub, token, claims.sdk_version, AuthMethod::Jwt)
                    }
                    AuthMethod::ApiKey => SdkCredentials::new(
                        "api-key-client".to_string(),
                        token,
                        "unknown".to_string(),
                        AuthMethod::ApiKey,
                    ),
                    AuthMethod::SdkSignature { sdk_id, signature } => SdkCredentials::new(
                        sdk_id.clone(),
                        token,
                        "unknown".to_string(),
                        AuthMethod::SdkSignature {
                            sdk_id: sdk_id.clone(),
                            signature,
                        },
                    ),
                };

                // Send authentication response
                let response = Message::AuthResponse {
                    success: true,
                    error: None,
                };

                connection.send_message(response).await?;

                Ok(credentials)
            }
            _ => Err(ConnectError::Authentication(
                "Expected authentication message".to_string(),
            )),
        }
    }

    /// Handle ongoing messages
    async fn handle_messages(&self, connection: &Connection) -> Result<()> {
        let connection_id = connection.id().await;

        loop {
            // Receive message with timeout
            let message =
                match timeout(self.config.connection_timeout, connection.receive_message()).await {
                    Ok(Ok(Some(msg))) => msg,
                    Ok(Ok(None)) => {
                        debug!(connection_id = %connection_id, "Connection closed by client");
                        break;
                    }
                    Ok(Err(e)) => {
                        error!(
                            connection_id = %connection_id,
                            error = %e,
                            "Error receiving message"
                        );
                        continue;
                    }
                    Err(_) => {
                        warn!(connection_id = %connection_id, "Message receive timeout");
                        continue;
                    }
                };

            // Process message
            if let Err(e) = self.process_message(connection, message).await {
                error!(
                    connection_id = %connection_id,
                    error = %e,
                    "Failed to process message"
                );

                // Send error response
                let error_msg = Message::Error {
                    code: "PROCESSING_ERROR".to_string(),
                    message: e.to_string(),
                    details: None,
                };

                if let Err(send_err) = connection.send_message(error_msg).await {
                    error!(
                        connection_id = %connection_id,
                        error = %send_err,
                        "Failed to send error message"
                    );
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process individual messages
    async fn process_message(&self, connection: &Connection, message: Message) -> Result<()> {
        let connection_id = connection.id().await;

        match message {
            Message::Ping { timestamp } => {
                // Respond with pong
                let pong = Message::Pong { timestamp };
                connection.send_message(pong).await?;
            }

            Message::Event {
                event: _,
                message_id,
            } => {
                debug!(
                    connection_id = %connection_id,
                    message_id = %message_id,
                    "Received event"
                );

                // TODO: Process event with event system

                // Send acknowledgment
                let ack = Message::EventAck {
                    message_id,
                    success: true,
                    error: None,
                };
                connection.send_message(ack).await?;
            }

            Message::FunctionExecution {
                function,
                context: _,
                message_id,
            } => {
                debug!(
                    connection_id = %connection_id,
                    message_id = %message_id,
                    function_name = %function.name,
                    "Received function execution request"
                );

                // TODO: Execute function with execution engine

                // Send response
                let response = Message::FunctionExecutionResponse {
                    message_id,
                    result: crate::protocol::ExecutionResult::Success {
                        data: serde_json::json!({"result": "success"}),
                        duration_ms: 100,
                    },
                };
                connection.send_message(response).await?;
            }

            Message::Close { reason, code } => {
                info!(
                    connection_id = %connection_id,
                    reason = %reason,
                    code = code,
                    "Received close message"
                );
                return Ok(());
            }

            _ => {
                warn!(
                    connection_id = %connection_id,
                    message_type = message.message_type(),
                    "Unhandled message type"
                );
            }
        }

        Ok(())
    }

    /// Convert Axum WebSocket to tokio-tungstenite WebSocketStream
    async fn convert_websocket(&self, _socket: WebSocket) -> Result<WebSocketStream<TcpStream>> {
        // This is a placeholder - in practice, you'd need to handle the conversion
        // For now, we'll return an error as this requires more complex implementation
        Err(ConnectError::Internal(
            "WebSocket conversion not implemented".to_string(),
        ))
    }

    /// Shutdown the server
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down connect server");
        self.manager.shutdown().await?;
        info!("Connect server shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::AuthService, config::AuthConfig, protocol::ProtocolNegotiator};
    use pretty_assertions::assert_eq;

    fn create_test_server() -> ConnectServer {
        let config = ConnectConfig::default();
        let auth_config = AuthConfig::default();
        let auth_service = AuthService::new(auth_config);
        let protocol_negotiator = ProtocolNegotiator::new(
            vec![ProtocolVersion::new(1, 0, None)],
            ProtocolVersion::new(1, 0, None),
        );

        ConnectServer::new(config, auth_service, protocol_negotiator)
    }

    #[test]
    fn test_server_creation() {
        let server = create_test_server();
        let stats = server.manager.get_stats();

        assert_eq!(stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_server_shutdown() {
        let server = create_test_server();
        let result = server.shutdown().await;
        assert!(result.is_ok());
    }
}
