//! WebSocket connection management

use crate::{
    auth::AuthContext,
    error::{ConnectError, Result},
    protocol::{Message, ProtocolVersion},
    types::{ClientInfo, ConnectionId, ConnectionStats, SessionId},
};
use chrono::{DateTime, Utc};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{
    net::TcpStream,
    sync::{Mutex, mpsc},
    time::{Duration, timeout},
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message as WsMessage, protocol::CloseFrame},
};
use tracing::{debug, error, info, warn};

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,

    /// Handshake in progress
    Handshaking,

    /// Authentication in progress
    Authenticating,

    /// Connection is active and ready
    Active,

    /// Connection is being closed
    Closing,

    /// Connection is closed
    Closed,

    /// Connection encountered an error
    Error(String),
}

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Unique connection identifier
    pub id: ConnectionId,

    /// Session identifier
    pub session_id: SessionId,

    /// Current connection state
    pub state: ConnectionState,

    /// Protocol version being used
    pub protocol_version: Option<ProtocolVersion>,

    /// Authentication context
    pub auth_context: Option<AuthContext>,

    /// Client information
    pub client_info: Option<ClientInfo>,

    /// Connection statistics
    pub stats: ConnectionStats,

    /// When the connection was created
    pub created_at: DateTime<Utc>,

    /// Remote peer address
    pub remote_addr: String,
}

impl ConnectionInfo {
    /// Create new connection info
    pub fn new(id: ConnectionId, remote_addr: String) -> Self {
        Self {
            id,
            session_id: SessionId::new(),
            state: ConnectionState::Connecting,
            protocol_version: None,
            auth_context: None,
            client_info: None,
            stats: ConnectionStats::new(),
            created_at: Utc::now(),
            remote_addr,
        }
    }

    /// Update connection state
    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
        self.stats.update_activity();
    }

    /// Set protocol version
    pub fn set_protocol_version(&mut self, version: ProtocolVersion) {
        self.protocol_version = Some(version);
    }

    /// Set authentication context
    pub fn set_auth_context(&mut self, auth_context: AuthContext) {
        self.auth_context = Some(auth_context);
    }

    /// Set client information
    pub fn set_client_info(&mut self, client_info: ClientInfo) {
        self.client_info = Some(client_info);
    }

    /// Check if connection is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.auth_context.is_some()
    }

    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        matches!(self.state, ConnectionState::Active)
    }

    /// Check if connection is closed
    pub fn is_closed(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Closed | ConnectionState::Error(_)
        )
    }
}

/// WebSocket connection wrapper
#[derive(Debug, Clone)]
pub struct Connection {
    /// Connection information
    info: Arc<Mutex<ConnectionInfo>>,
    sender: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, WsMessage>>>,

    /// Message receiver channel
    message_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Message>>>,

    /// Close sender channel
    close_sender: mpsc::UnboundedSender<CloseFrame<'static>>,
}

impl Connection {
    /// Create a new connection from a WebSocket stream
    pub fn new(
        id: ConnectionId,
        remote_addr: String,
        ws_stream: WebSocketStream<TcpStream>,
    ) -> (Self, ConnectionHandle) {
        let info = Arc::new(Mutex::new(ConnectionInfo::new(id, remote_addr)));
        let (ws_sender, ws_receiver) = ws_stream.split();
        let sender = Arc::new(Mutex::new(ws_sender));

        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (close_tx, close_rx) = mpsc::unbounded_channel();

        let message_receiver = Arc::new(Mutex::new(message_rx));

        let connection = Self {
            info: info.clone(),
            sender: sender.clone(),
            message_receiver,
            close_sender: close_tx,
        };

        let handle = ConnectionHandle {
            info,
            sender,
            message_sender: message_tx,
            close_receiver: Arc::new(Mutex::new(close_rx)),
            ws_receiver: Arc::new(Mutex::new(ws_receiver)),
        };

        (connection, handle)
    }

    /// Get connection information
    pub async fn info(&self) -> ConnectionInfo {
        self.info.lock().await.clone()
    }

    /// Update connection state
    pub async fn set_state(&self, state: ConnectionState) {
        let mut info = self.info.lock().await;
        info.set_state(state);
    }

    /// Set protocol version
    pub async fn set_protocol_version(&self, version: ProtocolVersion) {
        let mut info = self.info.lock().await;
        info.set_protocol_version(version);
    }

    /// Set authentication context
    pub async fn set_auth_context(&self, auth_context: AuthContext) {
        let mut info = self.info.lock().await;
        info.set_auth_context(auth_context);
    }

    /// Set client information
    pub async fn set_client_info(&self, client_info: ClientInfo) {
        let mut info = self.info.lock().await;
        info.set_client_info(client_info);
    }

    /// Send a message to the client
    pub async fn send_message(&self, message: Message) -> Result<()> {
        let json = message.to_json()?;
        let ws_message = WsMessage::Text(json);

        let message_len = ws_message.len() as u64;
        let mut sender = self.sender.lock().await;
        sender
            .send(ws_message)
            .await
            .map_err(|e| ConnectError::WebSocket(e.to_string()))?;

        // Update statistics
        let mut info = self.info.lock().await;
        info.stats.record_sent_message(message_len);

        debug!(
            connection_id = %info.id,
            message_type = message.message_type(),
            "Sent message to client"
        );

        Ok(())
    }

    /// Receive a message from the client
    pub async fn receive_message(&self) -> Result<Option<Message>> {
        let mut receiver = self.message_receiver.lock().await;
        Ok(receiver.recv().await)
    }

    /// Receive a message with timeout
    pub async fn receive_message_timeout(&self, duration: Duration) -> Result<Option<Message>> {
        let mut receiver = self.message_receiver.lock().await;
        match timeout(duration, receiver.recv()).await {
            Ok(message) => Ok(message),
            Err(_) => Err(ConnectError::Timeout("Message receive timeout".to_string())),
        }
    }

    /// Send a ping message
    pub async fn ping(&self) -> Result<()> {
        let ping_message = Message::Ping {
            timestamp: Utc::now(),
        };
        self.send_message(ping_message).await
    }

    /// Close the connection
    pub async fn close(&self, reason: String, code: u16) -> Result<()> {
        // Send close message to client
        let close_message = Message::Close {
            reason: reason.clone(),
            code,
        };

        if let Err(e) = self.send_message(close_message).await {
            warn!(
                connection_id = %self.info().await.id,
                error = %e,
                "Failed to send close message"
            );
        }

        // Send close frame
        let close_frame = CloseFrame {
            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
            reason: reason.into(),
        };

        if let Err(e) = self.close_sender.send(close_frame) {
            warn!(
                connection_id = %self.info().await.id,
                error = %e,
                "Failed to send close frame"
            );
        }

        // Update state
        self.set_state(ConnectionState::Closing).await;

        Ok(())
    }

    /// Get connection ID
    pub async fn id(&self) -> ConnectionId {
        self.info.lock().await.id
    }

    /// Get session ID
    pub async fn session_id(&self) -> SessionId {
        self.info.lock().await.session_id
    }

    /// Check if connection is authenticated
    pub async fn is_authenticated(&self) -> bool {
        self.info.lock().await.is_authenticated()
    }

    /// Check if connection is active
    pub async fn is_active(&self) -> bool {
        self.info.lock().await.is_active()
    }
}

/// Connection handle for managing the WebSocket stream
pub struct ConnectionHandle {
    /// Connection information
    info: Arc<Mutex<ConnectionInfo>>,

    /// WebSocket sender
    sender: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, WsMessage>>>,

    /// Message sender channel
    message_sender: mpsc::UnboundedSender<Message>,

    /// Close receiver channel
    close_receiver: Arc<Mutex<mpsc::UnboundedReceiver<CloseFrame<'static>>>>,

    /// WebSocket receiver
    ws_receiver: Arc<Mutex<SplitStream<WebSocketStream<TcpStream>>>>,
}

impl ConnectionHandle {
    /// Run the connection handler
    pub async fn run(self) -> Result<()> {
        let connection_id = self.info.lock().await.id;
        info!(connection_id = %connection_id, "Starting connection handler");

        // Spawn receiver task
        let receiver_handle = {
            let info = self.info.clone();
            let message_sender = self.message_sender.clone();
            let ws_receiver = self.ws_receiver.clone();

            tokio::spawn(async move {
                Self::handle_incoming_messages(info, message_sender, ws_receiver).await
            })
        };

        // Spawn close handler task
        let close_handle = {
            let info = self.info.clone();
            let sender = self.sender.clone();
            let close_receiver = self.close_receiver.clone();

            tokio::spawn(
                async move { Self::handle_close_requests(info, sender, close_receiver).await },
            )
        };

        // Wait for either task to complete
        tokio::select! {
            result = receiver_handle => {
                match result {
                    Ok(Ok(())) => info!(connection_id = %connection_id, "Receiver task completed successfully"),
                    Ok(Err(e)) => error!(connection_id = %connection_id, error = %e, "Receiver task failed"),
                    Err(e) => error!(connection_id = %connection_id, error = %e, "Receiver task panicked"),
                }
            }
            result = close_handle => {
                match result {
                    Ok(Ok(())) => info!(connection_id = %connection_id, "Close handler completed successfully"),
                    Ok(Err(e)) => error!(connection_id = %connection_id, error = %e, "Close handler failed"),
                    Err(e) => error!(connection_id = %connection_id, error = %e, "Close handler panicked"),
                }
            }
        }

        // Update connection state to closed
        let mut info = self.info.lock().await;
        info.set_state(ConnectionState::Closed);

        info!(connection_id = %connection_id, "Connection handler finished");
        Ok(())
    }

    /// Handle incoming WebSocket messages
    async fn handle_incoming_messages(
        info: Arc<Mutex<ConnectionInfo>>,
        message_sender: mpsc::UnboundedSender<Message>,
        ws_receiver: Arc<Mutex<SplitStream<WebSocketStream<TcpStream>>>>,
    ) -> Result<()> {
        let mut receiver = ws_receiver.lock().await;
        let connection_id = info.lock().await.id;

        while let Some(ws_message) = receiver.next().await {
            match ws_message {
                Ok(WsMessage::Text(text)) => {
                    // Update statistics
                    {
                        let mut info_guard = info.lock().await;
                        info_guard.stats.record_received_message(text.len() as u64);
                    }

                    // Parse message
                    match Message::from_json(&text) {
                        Ok(message) => {
                            debug!(
                                connection_id = %connection_id,
                                message_type = message.message_type(),
                                "Received message from client"
                            );

                            if let Err(e) = message_sender.send(message) {
                                error!(
                                    connection_id = %connection_id,
                                    error = %e,
                                    "Failed to forward message"
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            error!(
                                connection_id = %connection_id,
                                error = %e,
                                text = %text,
                                "Failed to parse message"
                            );

                            let mut info_guard = info.lock().await;
                            info_guard.stats.record_error();
                        }
                    }
                }
                Ok(WsMessage::Binary(_)) => {
                    warn!(
                        connection_id = %connection_id,
                        "Received binary message (not supported)"
                    );
                }
                Ok(WsMessage::Ping(_data)) => {
                    debug!(
                        connection_id = %connection_id,
                        "Received ping, sending pong"
                    );
                    // WebSocket library handles pong automatically
                }
                Ok(WsMessage::Pong(_)) => {
                    debug!(
                        connection_id = %connection_id,
                        "Received pong"
                    );
                }
                Ok(WsMessage::Close(close_frame)) => {
                    info!(
                        connection_id = %connection_id,
                        close_code = ?close_frame.as_ref().map(|f| f.code),
                        close_reason = ?close_frame.as_ref().map(|f| &f.reason),
                        "Received close frame"
                    );
                    break;
                }
                Ok(WsMessage::Frame(_)) => {
                    // Raw frame handling - usually not needed in application code
                    debug!(
                        connection_id = %connection_id,
                        "Received raw frame"
                    );
                }
                Err(e) => {
                    error!(
                        connection_id = %connection_id,
                        error = %e,
                        "WebSocket error"
                    );

                    let mut info_guard = info.lock().await;
                    info_guard.stats.record_error();
                    info_guard.set_state(ConnectionState::Error(e.to_string()));
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle close requests
    async fn handle_close_requests(
        info: Arc<Mutex<ConnectionInfo>>,
        sender: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, WsMessage>>>,
        close_receiver: Arc<Mutex<mpsc::UnboundedReceiver<CloseFrame<'static>>>>,
    ) -> Result<()> {
        let mut receiver = close_receiver.lock().await;
        let connection_id = info.lock().await.id;

        if let Some(close_frame) = receiver.recv().await {
            info!(
                connection_id = %connection_id,
                close_code = ?close_frame.code,
                close_reason = %close_frame.reason,
                "Sending close frame"
            );

            let mut sender_guard = sender.lock().await;
            if let Err(e) = sender_guard.send(WsMessage::Close(Some(close_frame))).await {
                error!(
                    connection_id = %connection_id,
                    error = %e,
                    "Failed to send close frame"
                );
            }

            if let Err(e) = sender_guard.close().await {
                error!(
                    connection_id = %connection_id,
                    error = %e,
                    "Failed to close WebSocket sender"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_connection_info() {
        let id = ConnectionId::new();
        let mut info = ConnectionInfo::new(id, "127.0.0.1:8080".to_string());

        assert_eq!(info.id, id);
        assert_eq!(info.state, ConnectionState::Connecting);
        assert!(!info.is_authenticated());
        assert!(!info.is_active());
        assert!(!info.is_closed());

        info.set_state(ConnectionState::Active);
        assert!(info.is_active());

        info.set_state(ConnectionState::Closed);
        assert!(info.is_closed());
    }

    #[test]
    fn test_connection_state() {
        let states = vec![
            ConnectionState::Connecting,
            ConnectionState::Handshaking,
            ConnectionState::Authenticating,
            ConnectionState::Active,
            ConnectionState::Closing,
            ConnectionState::Closed,
            ConnectionState::Error("test error".to_string()),
        ];

        for state in states {
            // Test serialization/deserialization
            let json = serde_json::to_string(&state).unwrap();
            let deserialized: ConnectionState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, deserialized);
        }
    }

    #[tokio::test]
    async fn test_connection_id() {
        let id1 = ConnectionId::new();
        let id2 = ConnectionId::new();

        assert_ne!(id1, id2);
        assert_eq!(id1.to_string().len(), 36); // UUID string length
    }
}
