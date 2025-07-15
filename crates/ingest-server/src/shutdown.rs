use crate::error::{Result, ServerError};
use crate::services::ServiceOrchestrator;
use futures::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast};
use tracing::{error, info, warn};

/// Shutdown coordinator for graceful service shutdown
pub struct ShutdownCoordinator {
    orchestrator: Arc<ServiceOrchestrator>,
    shutdown_timeout: Duration,
    force_shutdown_timeout: Duration,
    shutdown_sender: broadcast::Sender<ShutdownSignal>,
    _shutdown_receiver: broadcast::Receiver<ShutdownSignal>,
}

/// Types of shutdown signals
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    Graceful,
    Immediate,
    Force,
}

/// Shutdown handle for external shutdown requests
#[derive(Clone)]
pub struct ShutdownHandle {
    sender: broadcast::Sender<ShutdownSignal>,
}

impl ShutdownHandle {
    /// Request graceful shutdown
    pub fn shutdown_graceful(&self) -> Result<()> {
        self.sender
            .send(ShutdownSignal::Graceful)
            .map_err(|_| ServerError::Signal("Failed to send shutdown signal".to_string()))?;
        Ok(())
    }

    /// Request immediate shutdown
    pub fn shutdown_immediate(&self) -> Result<()> {
        self.sender
            .send(ShutdownSignal::Immediate)
            .map_err(|_| ServerError::Signal("Failed to send shutdown signal".to_string()))?;
        Ok(())
    }

    /// Request force shutdown
    pub fn shutdown_force(&self) -> Result<()> {
        self.sender
            .send(ShutdownSignal::Force)
            .map_err(|_| ServerError::Signal("Failed to send shutdown signal".to_string()))?;
        Ok(())
    }

    /// Subscribe to shutdown signals
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownSignal> {
        self.sender.subscribe()
    }
}

impl ShutdownCoordinator {
    pub fn new(orchestrator: Arc<ServiceOrchestrator>) -> Self {
        let (shutdown_sender, shutdown_receiver) = broadcast::channel(16);

        Self {
            orchestrator,
            shutdown_timeout: Duration::from_secs(30),
            force_shutdown_timeout: Duration::from_secs(10),
            shutdown_sender,
            _shutdown_receiver: shutdown_receiver,
        }
    }

    pub fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    pub fn with_force_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.force_shutdown_timeout = timeout;
        self
    }

    /// Get a handle for external shutdown requests
    pub fn handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            sender: self.shutdown_sender.clone(),
        }
    }

    /// Start listening for shutdown signals
    pub async fn listen_for_signals(&self) -> Result<()> {
        let mut signals = Signals::new([SIGTERM, SIGINT, SIGQUIT])
            .map_err(|e| ServerError::Signal(e.to_string()))?;

        let shutdown_sender = self.shutdown_sender.clone();

        tokio::spawn(async move {
            while let Some(signal) = signals.next().await {
                match signal {
                    SIGTERM => {
                        info!("Received SIGTERM, initiating graceful shutdown");
                        let _ = shutdown_sender.send(ShutdownSignal::Graceful);
                    }
                    SIGINT => {
                        info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
                        let _ = shutdown_sender.send(ShutdownSignal::Graceful);
                    }
                    SIGQUIT => {
                        warn!("Received SIGQUIT, initiating immediate shutdown");
                        let _ = shutdown_sender.send(ShutdownSignal::Immediate);
                    }
                    _ => {
                        warn!("Received unknown signal: {}", signal);
                    }
                }
            }
        });

        Ok(())
    }

    /// Wait for shutdown signal and coordinate shutdown
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        let mut receiver = self.shutdown_sender.subscribe();

        loop {
            match receiver.recv().await {
                Ok(ShutdownSignal::Graceful) => {
                    info!("Graceful shutdown requested");
                    return self.shutdown_gracefully().await;
                }
                Ok(ShutdownSignal::Immediate) => {
                    warn!("Immediate shutdown requested");
                    return self.shutdown_immediately().await;
                }
                Ok(ShutdownSignal::Force) => {
                    error!("Force shutdown requested");
                    return self.shutdown_force().await;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    warn!("Shutdown signal receiver lagged, continuing");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    error!("Shutdown signal channel closed");
                    return Err(ServerError::Signal("Shutdown channel closed".to_string()));
                }
            }
        }
    }

    /// Perform graceful shutdown
    pub async fn shutdown_gracefully(&self) -> Result<()> {
        info!("Initiating graceful shutdown");

        // Phase 1: Stop accepting new requests
        info!("Phase 1: Stopping new request acceptance");
        self.stop_accepting_requests().await?;

        // Phase 2: Drain existing requests
        info!("Phase 2: Draining existing requests");
        self.drain_requests().await?;

        // Phase 3: Stop services gracefully
        info!("Phase 3: Stopping services gracefully");
        let shutdown_result =
            tokio::time::timeout(self.shutdown_timeout, self.orchestrator.stop_all()).await;

        match shutdown_result {
            Ok(Ok(_)) => {
                info!("Graceful shutdown completed successfully");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Graceful shutdown failed: {:?}", e);
                warn!("Falling back to immediate shutdown");
                self.shutdown_immediately().await
            }
            Err(_) => {
                error!("Graceful shutdown timed out");
                warn!("Falling back to immediate shutdown");
                self.shutdown_immediately().await
            }
        }
    }

    /// Perform immediate shutdown
    pub async fn shutdown_immediately(&self) -> Result<()> {
        warn!("Initiating immediate shutdown");

        let shutdown_result =
            tokio::time::timeout(self.force_shutdown_timeout, self.orchestrator.stop_all()).await;

        match shutdown_result {
            Ok(Ok(_)) => {
                warn!("Immediate shutdown completed");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Immediate shutdown failed: {:?}", e);
                error!("Falling back to force shutdown");
                self.shutdown_force().await
            }
            Err(_) => {
                error!("Immediate shutdown timed out");
                error!("Falling back to force shutdown");
                self.shutdown_force().await
            }
        }
    }

    /// Perform force shutdown
    pub async fn shutdown_force(&self) -> Result<()> {
        error!("Initiating force shutdown");

        // Force stop all services without timeout
        self.orchestrator.force_stop_all().await?;

        error!("Force shutdown completed");
        Ok(())
    }

    /// Stop accepting new requests
    async fn stop_accepting_requests(&self) -> Result<()> {
        // In a real implementation, this would signal the API and Connect services
        // to stop accepting new connections and requests

        // For now, we'll just log and simulate the operation
        info!("Signaling services to stop accepting new requests");

        // Give services time to stop accepting new requests
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Drain existing requests
    async fn drain_requests(&self) -> Result<()> {
        // In a real implementation, this would wait for existing requests to complete
        // and monitor active connections

        info!("Waiting for existing requests to complete");

        let drain_timeout = Duration::from_secs(10);
        let start = std::time::Instant::now();

        while start.elapsed() < drain_timeout {
            // Check if there are any active requests/connections
            let active_requests = self.count_active_requests().await;

            if active_requests == 0 {
                info!("All requests drained successfully");
                return Ok(());
            }

            info!(
                "Waiting for {} active requests to complete",
                active_requests
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        warn!("Request drain timeout exceeded, proceeding with shutdown");
        Ok(())
    }

    /// Count active requests across all services
    async fn count_active_requests(&self) -> usize {
        // In a real implementation, this would query the API and Connect services
        // for their active request/connection counts

        // For now, we'll simulate this by returning 0 after a short delay
        0
    }
}

/// Shutdown manager for coordinating shutdown across the entire server
pub struct ShutdownManager {
    coordinator: Arc<ShutdownCoordinator>,
    shutdown_state: Arc<RwLock<ShutdownState>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ShutdownState {
    Running,
    ShuttingDown,
    Shutdown,
}

impl ShutdownManager {
    pub fn new(coordinator: Arc<ShutdownCoordinator>) -> Self {
        Self {
            coordinator,
            shutdown_state: Arc::new(RwLock::new(ShutdownState::Running)),
        }
    }

    /// Start the shutdown manager
    pub async fn start(&self) -> Result<()> {
        // Start listening for signals
        self.coordinator.listen_for_signals().await?;

        info!("Shutdown manager started");
        Ok(())
    }

    /// Wait for shutdown and coordinate the process
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        // Update state to shutting down
        {
            let mut state = self.shutdown_state.write().await;
            *state = ShutdownState::ShuttingDown;
        }

        // Wait for and handle shutdown
        let result = self.coordinator.wait_for_shutdown().await;

        // Update state to shutdown
        {
            let mut state = self.shutdown_state.write().await;
            *state = ShutdownState::Shutdown;
        }

        result
    }

    /// Get shutdown handle for external use
    pub fn handle(&self) -> ShutdownHandle {
        self.coordinator.handle()
    }

    /// Check if shutdown is in progress
    pub async fn is_shutting_down(&self) -> bool {
        let state = self.shutdown_state.read().await;
        matches!(
            *state,
            ShutdownState::ShuttingDown | ShutdownState::Shutdown
        )
    }

    /// Check if shutdown is complete
    pub async fn is_shutdown(&self) -> bool {
        let state = self.shutdown_state.read().await;
        matches!(*state, ShutdownState::Shutdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{Service, ServiceOrchestrator};
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockService {
        name: String,
        started: Arc<AtomicBool>,
    }

    impl MockService {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                started: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[async_trait]
    impl Service for MockService {
        async fn start(&self) -> Result<()> {
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> Result<()> {
            self.started.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn health_check(&self) -> crate::services::HealthStatus {
            crate::services::HealthStatus::Healthy
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_shutdown_handle() {
        let mut orchestrator = ServiceOrchestrator::new();
        orchestrator.register_service(Box::new(MockService::new("test")));

        let coordinator = Arc::new(ShutdownCoordinator::new(Arc::new(orchestrator)));
        let fixture = coordinator.handle();

        let actual = fixture.shutdown_graceful().is_ok();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_graceful() {
        let mut orchestrator = ServiceOrchestrator::new();
        orchestrator.register_service(Box::new(MockService::new("test")));

        let fixture = ShutdownCoordinator::new(Arc::new(orchestrator));

        let actual = fixture.shutdown_gracefully().await.is_ok();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_shutdown_manager_lifecycle() {
        let mut orchestrator = ServiceOrchestrator::new();
        orchestrator.register_service(Box::new(MockService::new("test")));

        let coordinator = Arc::new(ShutdownCoordinator::new(Arc::new(orchestrator)));
        let fixture = ShutdownManager::new(coordinator);

        let actual = fixture.is_shutting_down().await;
        let expected = false;
        assert_eq!(actual, expected);

        let actual = fixture.is_shutdown().await;
        let expected = false;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_shutdown_signal_types() {
        let fixture = ShutdownSignal::Graceful;
        let actual = fixture == ShutdownSignal::Graceful;
        let expected = true;
        assert_eq!(actual, expected);

        let fixture = ShutdownSignal::Immediate;
        let actual = fixture == ShutdownSignal::Immediate;
        let expected = true;
        assert_eq!(actual, expected);
    }
}
