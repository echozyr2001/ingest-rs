//! Connection manager and connection pooling

use crate::{
    auth::AuthService,
    config::{ConnectConfig, LoadBalancingStrategy},
    connection::{Connection, ConnectionState},
    error::{ConnectError, Result},
    protocol::Message,
    types::{ClientInfo, ConnectionId},
};
use dashmap::DashMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    sync::{RwLock, mpsc},
    time::interval,
};
use tracing::{debug, error, info, warn};

/// Connection manager for handling multiple WebSocket connections
#[derive(Clone)]
pub struct ConnectionManager {
    /// Configuration
    config: Arc<ConnectConfig>,

    /// Authentication service
    auth_service: AuthService,

    /// Active connections
    connections: Arc<DashMap<ConnectionId, Connection>>,

    /// Connection pools by session
    pools: Arc<DashMap<String, ConnectionPool>>,

    /// Connection statistics
    stats: Arc<ConnectionStats>,

    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<mpsc::UnboundedSender<()>>>>,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(config: ConnectConfig, auth_service: AuthService) -> Self {
        let (shutdown_tx, _) = mpsc::unbounded_channel();

        Self {
            config: Arc::new(config),
            auth_service,
            connections: Arc::new(DashMap::new()),
            pools: Arc::new(DashMap::new()),
            stats: Arc::new(ConnectionStats::new()),
            shutdown_tx: Arc::new(RwLock::new(Some(shutdown_tx))),
        }
    }

    /// Add a new connection
    pub async fn add_connection(&self, connection: Connection) -> Result<()> {
        let connection_id = connection.id().await;
        let info = connection.info().await;

        // Check connection limits
        if self.stats.active_connections.load(Ordering::Relaxed) >= self.config.max_connections {
            return Err(ConnectError::ConnectionPool(
                "Maximum connections exceeded".to_string(),
            ));
        }

        // Add to connections map
        self.connections.insert(connection_id, connection);
        self.stats
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
        self.stats.total_connections.fetch_add(1, Ordering::Relaxed);

        info!(
            connection_id = %connection_id,
            remote_addr = %info.remote_addr,
            "Connection added to manager"
        );

        Ok(())
    }

    /// Remove a connection
    pub async fn remove_connection(&self, connection_id: ConnectionId) -> Result<()> {
        if let Some((_, connection)) = self.connections.remove(&connection_id) {
            let info = connection.info().await;

            // Remove from pools if authenticated
            if let Some(auth_context) = &info.auth_context {
                if let Some(pool) = self.pools.get_mut(&auth_context.sdk_id) {
                    let _ = pool.remove_connection(connection_id).await;
                }
            }

            self.stats
                .active_connections
                .fetch_sub(1, Ordering::Relaxed);

            info!(
                connection_id = %connection_id,
                "Connection removed from manager"
            );
        }

        Ok(())
    }

    /// Get a connection by ID
    pub fn get_connection(&self, connection_id: ConnectionId) -> Option<Connection> {
        self.connections
            .get(&connection_id)
            .map(|entry| (*entry.value()).clone())
    }

    /// Get all connections for an SDK
    pub async fn get_sdk_connections(&self, sdk_id: &str) -> Vec<Connection> {
        if let Some(pool) = self.pools.get(sdk_id) {
            pool.get_all_connections().await
        } else {
            Vec::new()
        }
    }

    /// Authenticate a connection
    pub async fn authenticate_connection(
        &self,
        connection_id: ConnectionId,
        credentials: &crate::auth::SdkCredentials,
        client_info: ClientInfo,
    ) -> Result<()> {
        let connection = self
            .get_connection(connection_id)
            .ok_or_else(|| ConnectError::ConnectionNotFound(connection_id.to_string()))?;

        // Authenticate with auth service
        let auth_context = self
            .auth_service
            .authenticate_sdk(credentials, client_info.clone())?;

        // Update connection
        connection.set_auth_context(auth_context.clone()).await;
        connection.set_client_info(client_info).await;
        connection.set_state(ConnectionState::Active).await;

        // Add to pool
        let pool = self
            .pools
            .entry(auth_context.sdk_id.clone())
            .or_insert_with(|| {
                ConnectionPool::new(auth_context.sdk_id.clone(), self.config.pool.clone())
            });

        pool.add_connection(connection).await?;

        info!(
            connection_id = %connection_id,
            sdk_id = %auth_context.sdk_id,
            "Connection authenticated and added to pool"
        );

        Ok(())
    }

    /// Broadcast message to all connections of an SDK
    pub async fn broadcast_to_sdk(&self, sdk_id: &str, message: Message) -> Result<usize> {
        let connections = self.get_sdk_connections(sdk_id).await;
        let mut sent_count = 0;

        for connection in connections {
            if connection.is_active().await {
                if let Err(e) = connection.send_message(message.clone()).await {
                    warn!(
                        connection_id = %connection.id().await,
                        error = %e,
                        "Failed to send broadcast message"
                    );
                } else {
                    sent_count += 1;
                }
            }
        }

        debug!(
            sdk_id = %sdk_id,
            sent_count = sent_count,
            "Broadcast message sent"
        );

        Ok(sent_count)
    }

    /// Get connection statistics
    pub fn get_stats(&self) -> ConnectionManagerStats {
        ConnectionManagerStats {
            active_connections: self.stats.active_connections.load(Ordering::Relaxed),
            total_connections: self.stats.total_connections.load(Ordering::Relaxed),
            pool_count: self.pools.len(),
        }
    }

    /// Start background tasks
    pub async fn start_background_tasks(&self) -> Result<()> {
        let manager = self.clone();

        // Start cleanup task
        tokio::spawn(async move {
            manager.cleanup_task().await;
        });

        // Start health check task
        let manager = self.clone();
        tokio::spawn(async move {
            manager.health_check_task().await;
        });

        Ok(())
    }

    /// Cleanup task for removing stale connections
    async fn cleanup_task(&self) {
        let mut interval = interval(self.config.pool.cleanup_interval);

        loop {
            interval.tick().await;

            // Check for shutdown
            if self.shutdown_tx.read().await.is_none() {
                break;
            }

            let mut to_remove = Vec::new();

            // Find stale connections
            for entry in self.connections.iter() {
                let connection = entry.value();
                let info = connection.info().await;

                if info.is_closed()
                    || info.stats.idle_duration()
                        > chrono::Duration::from_std(self.config.pool.idle_timeout)
                            .unwrap_or_default()
                {
                    to_remove.push(info.id);
                }
            }

            // Remove stale connections
            for connection_id in to_remove {
                if let Err(e) = self.remove_connection(connection_id).await {
                    error!(
                        connection_id = %connection_id,
                        error = %e,
                        "Failed to remove stale connection"
                    );
                }
            }

            debug!("Cleanup task completed");
        }
    }

    /// Health check task for monitoring connections
    async fn health_check_task(&self) {
        let mut interval = interval(self.config.heartbeat_interval);

        loop {
            interval.tick().await;

            // Check for shutdown
            if self.shutdown_tx.read().await.is_none() {
                break;
            }

            // Send ping to all active connections
            for entry in self.connections.iter() {
                let connection = entry.value();

                if connection.is_active().await {
                    if let Err(e) = connection.ping().await {
                        warn!("Failed to send ping to connection: {}", e);
                    }
                }
            }

            debug!("Health check completed");
        }
    }

    /// Shutdown the connection manager
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down connection manager");

        // Signal shutdown
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }

        // Close all connections
        let connection_ids: Vec<ConnectionId> =
            self.connections.iter().map(|entry| *entry.key()).collect();

        for connection_id in connection_ids {
            if let Some(connection) = self.get_connection(connection_id) {
                if let Err(e) = connection.close("Server shutdown".to_string(), 1001).await {
                    warn!(
                        connection_id = %connection_id,
                        error = %e,
                        "Failed to close connection during shutdown"
                    );
                }
            }
        }

        info!("Connection manager shutdown complete");
        Ok(())
    }
}

/// Connection pool for managing connections from a specific SDK
#[derive(Clone)]
pub struct ConnectionPool {
    /// SDK identifier
    sdk_id: String,

    /// Pool configuration
    config: crate::config::PoolConfig,

    /// Connections in the pool
    connections: Arc<DashMap<ConnectionId, Connection>>,

    /// Load balancer state
    load_balancer: Arc<LoadBalancer>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(sdk_id: String, config: crate::config::PoolConfig) -> Self {
        Self {
            sdk_id,
            config: config.clone(),
            connections: Arc::new(DashMap::new()),
            load_balancer: Arc::new(LoadBalancer::new(config.load_balancing)),
        }
    }

    /// Add a connection to the pool
    pub async fn add_connection(&self, connection: Connection) -> Result<()> {
        let connection_id = connection.id().await;

        // Check pool limits
        if self.connections.len() >= self.config.max_pool_size {
            return Err(ConnectError::ConnectionPool(format!(
                "Pool {} is at maximum capacity",
                self.sdk_id
            )));
        }

        self.connections.insert(connection_id, connection);
        self.load_balancer.add_connection(connection_id).await;

        debug!(
            sdk_id = %self.sdk_id,
            connection_id = %connection_id,
            pool_size = self.connections.len(),
            "Connection added to pool"
        );

        Ok(())
    }

    /// Remove a connection from the pool
    pub async fn remove_connection(&self, connection_id: ConnectionId) -> Result<()> {
        if self.connections.remove(&connection_id).is_some() {
            self.load_balancer.remove_connection(connection_id).await;

            debug!(
                sdk_id = %self.sdk_id,
                connection_id = %connection_id,
                pool_size = self.connections.len(),
                "Connection removed from pool"
            );
        }

        Ok(())
    }

    /// Get a connection using load balancing
    pub async fn get_connection(&self) -> Option<Connection> {
        let connection_id = self.load_balancer.next_connection().await?;
        self.connections
            .get(&connection_id)
            .map(|entry| (*entry.value()).clone())
    }

    /// Get all connections in the pool
    pub async fn get_all_connections(&self) -> Vec<Connection> {
        self.connections
            .iter()
            .map(|entry| (*entry.value()).clone())
            .collect()
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        PoolStats {
            sdk_id: self.sdk_id.clone(),
            connection_count: self.connections.len(),
            max_pool_size: self.config.max_pool_size,
        }
    }
}

/// Load balancer for connection pools
struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    connections: Arc<RwLock<Vec<ConnectionId>>>,
    current_index: Arc<AtomicUsize>,
}

impl LoadBalancer {
    fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            connections: Arc::new(RwLock::new(Vec::new())),
            current_index: Arc::new(AtomicUsize::new(0)),
        }
    }

    async fn add_connection(&self, connection_id: ConnectionId) {
        let mut connections = self.connections.write().await;
        connections.push(connection_id);
    }

    async fn remove_connection(&self, connection_id: ConnectionId) {
        let mut connections = self.connections.write().await;
        connections.retain(|&id| id != connection_id);
    }

    async fn next_connection(&self) -> Option<ConnectionId> {
        let connections = self.connections.read().await;

        if connections.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let index = self.current_index.fetch_add(1, Ordering::Relaxed) % connections.len();
                Some(connections[index])
            }
            LoadBalancingStrategy::Random => {
                use rand::Rng;
                let index = rand::thread_rng().gen_range(0..connections.len());
                Some(connections[index])
            }
            LoadBalancingStrategy::LeastConnections | LoadBalancingStrategy::WeightedRoundRobin => {
                // For now, fall back to round-robin
                // TODO: Implement proper least connections and weighted round-robin
                let index = self.current_index.fetch_add(1, Ordering::Relaxed) % connections.len();
                Some(connections[index])
            }
        }
    }
}

/// Connection manager statistics
#[derive(Debug, Clone)]
pub struct ConnectionManagerStats {
    pub active_connections: usize,
    pub total_connections: usize,
    pub pool_count: usize,
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub sdk_id: String,
    pub connection_count: usize,
    pub max_pool_size: usize,
}

/// Internal connection statistics
struct ConnectionStats {
    active_connections: AtomicUsize,
    total_connections: AtomicUsize,
}

impl ConnectionStats {
    fn new() -> Self {
        Self {
            active_connections: AtomicUsize::new(0),
            total_connections: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::AuthService, config::AuthConfig};
    use pretty_assertions::assert_eq;

    fn create_test_manager() -> ConnectionManager {
        let config = ConnectConfig::default();
        let auth_config = AuthConfig::default();
        let auth_service = AuthService::new(auth_config);

        ConnectionManager::new(config, auth_service)
    }

    #[test]
    fn test_connection_manager_creation() {
        let manager = create_test_manager();
        let stats = manager.get_stats();

        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pool_count, 0);
    }

    #[test]
    fn test_connection_pool_creation() {
        let config = crate::config::PoolConfig::default();
        let pool = ConnectionPool::new("test-sdk".to_string(), config);
        let stats = pool.get_stats();

        assert_eq!(stats.sdk_id, "test-sdk");
        assert_eq!(stats.connection_count, 0);
    }

    #[test]
    fn test_load_balancer() {
        let _balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        // More detailed testing would require async runtime
    }
}
