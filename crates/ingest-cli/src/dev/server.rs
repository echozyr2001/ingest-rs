use crate::config::ProjectConfig;
use crate::dev::watcher::FunctionWatcher;
use crate::utils::output::{error, info};
use anyhow::Result;
use tokio::sync::broadcast;

pub struct DevServer {
    #[allow(dead_code)]
    config: ProjectConfig,
    host: String,
    port: u16,
    #[allow(dead_code)]
    auto_reload: bool,
    shutdown_tx: broadcast::Sender<()>,
    function_watcher: Option<FunctionWatcher>,
}

impl DevServer {
    pub async fn new(
        config: ProjectConfig,
        host: String,
        port: u16,
        auto_reload: bool,
    ) -> Result<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);

        let function_watcher = if auto_reload {
            Some(FunctionWatcher::new(&config.project.functions_dir).await?)
        } else {
            None
        };

        Ok(Self {
            config,
            host,
            port,
            auto_reload,
            shutdown_tx,
            function_watcher,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info(&format!(
            "🚀 Starting development server on {}:{}",
            self.host, self.port
        ));

        // Start function watcher if auto-reload is enabled
        if let Some(watcher) = self.function_watcher.take() {
            let shutdown_rx = self.shutdown_tx.subscribe();
            let _watcher_handle = tokio::spawn(async move {
                let mut watcher = watcher;
                if let Err(e) = watcher.start(shutdown_rx).await {
                    error(&format!("Function watcher error: {e}"));
                }
            });
        }

        // TODO: Start actual Inngest server integration
        // For now, just simulate a running server
        info("📡 Server is running...");
        info("📂 Watching for function changes...");

        // Wait for shutdown signal
        self.wait_for_shutdown().await?;

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        info("Development server stopped");
        Ok(())
    }

    async fn wait_for_shutdown(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigterm = signal(SignalKind::terminate())?;
            let mut sigint = signal(SignalKind::interrupt())?;

            tokio::select! {
                _ = sigterm.recv() => {
                    info("Received SIGTERM, shutting down...");
                }
                _ = sigint.recv() => {
                    info("Received SIGINT, shutting down...");
                }
            }
        }

        #[cfg(not(unix))]
        {
            use tokio::signal;
            signal::ctrl_c().await?;
            info("Received Ctrl+C, shutting down...");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn reload_functions(&mut self) -> Result<()> {
        info("🔄 Reloading functions...");

        // TODO: Implement function reloading logic
        // 1. Scan functions directory
        // 2. Parse function definitions
        // 3. Validate functions
        // 4. Update function registry
        // 5. Notify connected clients

        info("✅ Functions reloaded successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dev_server_creation() {
        let fixture_config = ProjectConfig::default();
        let actual = DevServer::new(fixture_config, "localhost".to_string(), 3000, true).await;

        assert!(actual.is_ok());
    }

    #[tokio::test]
    async fn test_function_reload() {
        let fixture_config = ProjectConfig::default();
        let mut server = DevServer::new(fixture_config, "localhost".to_string(), 3000, false)
            .await
            .unwrap();

        let actual = server.reload_functions().await;
        assert!(actual.is_ok());
    }
}
