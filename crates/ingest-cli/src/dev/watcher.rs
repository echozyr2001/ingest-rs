use crate::utils::output::{error, info};
use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone)]
pub enum WatchEvent {
    FunctionAdded(PathBuf),
    FunctionModified(PathBuf),
    FunctionRemoved(PathBuf),
    ConfigChanged(PathBuf),
}

pub struct FunctionWatcher {
    watcher: Option<RecommendedWatcher>,
    watch_path: PathBuf,
    event_tx: mpsc::Sender<WatchEvent>,
    event_rx: mpsc::Receiver<WatchEvent>,
}

impl FunctionWatcher {
    pub async fn new(functions_dir: &Path) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel(100);

        Ok(Self {
            watcher: None,
            watch_path: functions_dir.to_path_buf(),
            event_tx,
            event_rx,
        })
    }

    pub async fn start(&mut self, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
        info(&format!(
            "👀 Watching for changes in: {}",
            self.watch_path.display()
        ));

        let event_tx = self.event_tx.clone();
        let watch_path = self.watch_path.clone();

        // Create file system watcher
        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if let Err(e) = handle_fs_event(event, &event_tx, &watch_path) {
                        error(&format!("Error handling file system event: {e}"));
                    }
                }
                Err(e) => {
                    error(&format!("File system watcher error: {e}"));
                }
            })?;

        // Start watching the functions directory
        watcher.watch(&self.watch_path, RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);

        // Process events until shutdown
        loop {
            tokio::select! {
                Some(watch_event) = self.event_rx.recv() => {
                    if let Err(e) = self.handle_watch_event(watch_event).await {
                        error(&format!("Error processing watch event: {e}"));
                    }
                }
                _ = shutdown_rx.recv() => {
                    info("Stopping function watcher...");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_watch_event(&self, event: WatchEvent) -> Result<()> {
        match event {
            WatchEvent::FunctionAdded(path) => {
                info(&format!("📄 Function added: {}", path.display()));
                // TODO: Parse and register new function
            }
            WatchEvent::FunctionModified(path) => {
                info(&format!("📝 Function modified: {}", path.display()));
                // TODO: Reparse and update function
            }
            WatchEvent::FunctionRemoved(path) => {
                info(&format!("🗑️  Function removed: {}", path.display()));
                // TODO: Unregister function
            }
            WatchEvent::ConfigChanged(path) => {
                info(&format!("⚙️  Configuration changed: {}", path.display()));
                // TODO: Reload configuration
            }
        }

        // Add a small delay to debounce rapid file changes
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

fn handle_fs_event(
    event: Event,
    event_tx: &mpsc::Sender<WatchEvent>,
    _watch_path: &Path,
) -> Result<()> {
    use notify::EventKind;

    for path in event.paths {
        // Skip non-Rust files and hidden files
        if let Some(extension) = path.extension() {
            if extension != "rs" && extension != "toml" {
                continue;
            }
        }

        if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        let watch_event = match event.kind {
            EventKind::Create(_) => {
                if is_function_file(&path) {
                    WatchEvent::FunctionAdded(path)
                } else if is_config_file(&path) {
                    WatchEvent::ConfigChanged(path)
                } else {
                    continue;
                }
            }
            EventKind::Modify(_) => {
                if is_function_file(&path) {
                    WatchEvent::FunctionModified(path)
                } else if is_config_file(&path) {
                    WatchEvent::ConfigChanged(path)
                } else {
                    continue;
                }
            }
            EventKind::Remove(_) => {
                if is_function_file(&path) {
                    WatchEvent::FunctionRemoved(path)
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        // Send event asynchronously
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = event_tx.send(watch_event).await {
                error(&format!("Failed to send watch event: {e}"));
            }
        });
    }

    Ok(())
}

fn is_function_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "rs")
        .unwrap_or(false)
}

fn is_config_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "inngest.toml" || name == "Cargo.toml")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_watcher_creation() {
        let fixture = TempDir::new().unwrap();
        let actual = FunctionWatcher::new(fixture.path()).await;
        assert!(actual.is_ok());
    }

    #[test]
    fn test_is_function_file() {
        let fixture = Path::new("src/functions/hello.rs");
        let actual = is_function_file(fixture);
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_is_config_file() {
        let fixture = Path::new("inngest.toml");
        let actual = is_config_file(fixture);
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_not_function_file() {
        let fixture = Path::new("README.md");
        let actual = is_function_file(fixture);
        let expected = false;
        assert_eq!(actual, expected);
    }
}
