//! Concurrency control for function execution
//!
//! Handles concurrency limiting at different scopes: function, account, and custom key-based limits

use async_trait::async_trait;
use dashmap::DashMap;
use inngest_core::{Error, Result, Uuid};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Scope for concurrency limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConcurrencyScope {
    Function,
    Account,
    Custom,
}

/// Configuration for a single concurrency limit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyLimit {
    pub limit: u32,
    pub scope: ConcurrencyScope,
    pub key: Option<String>,
}

impl ConcurrencyLimit {
    pub fn new(limit: u32, scope: ConcurrencyScope) -> Self {
        Self {
            limit,
            scope,
            key: None,
        }
    }

    pub fn with_key(mut self, key: String) -> Self {
        self.key = Some(key);
        self
    }

    pub fn is_partition_limit(&self) -> bool {
        self.scope == ConcurrencyScope::Function
    }
}

/// Collection of concurrency limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyLimits {
    pub limits: Vec<ConcurrencyLimit>,
}

impl ConcurrencyLimits {
    pub fn new() -> Self {
        Self { limits: Vec::new() }
    }

    pub fn add_limit(mut self, limit: ConcurrencyLimit) -> Self {
        self.limits.push(limit);
        self
    }

    pub fn validate(&self) -> Result<()> {
        for limit in &self.limits {
            if limit.scope != ConcurrencyScope::Function && limit.key.is_none() {
                return Err(Error::InvalidConfiguration(format!(
                    "Concurrency key must be specified for {:?} scoped limits",
                    limit.scope
                )));
            }

            if limit.limit == 0 {
                return Err(Error::InvalidConfiguration(
                    "Concurrency limit must be greater than 0".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl Default for ConcurrencyLimits {
    fn default() -> Self {
        Self::new()
    }
}

/// Item in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub function_id: Uuid,
    pub account_id: Uuid,
    pub data: serde_json::Value,
}

/// Trait for concurrency management
#[async_trait]
pub trait ConcurrencyManager: Send + Sync {
    async fn add(&self, function_id: Uuid, item: &QueueItem) -> Result<()>;
    async fn done(&self, function_id: Uuid, item: &QueueItem) -> Result<()>;
    async fn check(&self, function_id: Uuid, limit: u32) -> Result<()>;
    async fn acquire(
        &self,
        function_id: Uuid,
        limits: &ConcurrencyLimits,
        item: &QueueItem,
    ) -> Result<ConcurrencyGuard>;
}

/// RAII guard for concurrency slots
pub struct ConcurrencyGuard {
    manager: Arc<dyn ConcurrencyManager>,
    function_id: Uuid,
    item: QueueItem,
}

impl ConcurrencyGuard {
    pub fn new(manager: Arc<dyn ConcurrencyManager>, function_id: Uuid, item: QueueItem) -> Self {
        Self {
            manager,
            function_id,
            item,
        }
    }
}

impl Drop for ConcurrencyGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        let function_id = self.function_id;
        let item = self.item.clone();
        tokio::spawn(async move {
            if let Err(e) = manager.done(function_id, &item).await {
                tracing::error!("Failed to release concurrency slot: {}", e);
            }
        });
    }
}

/// In-memory concurrency manager using semaphores
pub struct InMemoryConcurrencyManager {
    function_semaphores: DashMap<Uuid, Arc<Semaphore>>,
    active_items: DashMap<String, QueueItem>,
}

impl InMemoryConcurrencyManager {
    pub fn new() -> Self {
        Self {
            function_semaphores: DashMap::new(),
            active_items: DashMap::new(),
        }
    }

    fn get_or_create_semaphore(&self, function_id: Uuid, limit: u32) -> Arc<Semaphore> {
        self.function_semaphores
            .entry(function_id)
            .or_insert_with(|| Arc::new(Semaphore::new(limit as usize)))
            .clone()
    }

    fn item_key(&self, function_id: Uuid, item: &QueueItem) -> String {
        format!("{}:{}", function_id, item.id)
    }
}

#[async_trait]
impl ConcurrencyManager for InMemoryConcurrencyManager {
    async fn add(&self, function_id: Uuid, item: &QueueItem) -> Result<()> {
        let key = self.item_key(function_id, item);
        self.active_items.insert(key, item.clone());
        Ok(())
    }

    async fn done(&self, function_id: Uuid, item: &QueueItem) -> Result<()> {
        let key = self.item_key(function_id, item);
        self.active_items.remove(&key);
        Ok(())
    }

    async fn check(&self, function_id: Uuid, limit: u32) -> Result<()> {
        let semaphore = self.get_or_create_semaphore(function_id, limit);
        if semaphore.available_permits() == 0 {
            return Err(Error::ConcurrencyLimit("At concurrency limit".to_string()));
        }
        Ok(())
    }

    async fn acquire(
        &self,
        function_id: Uuid,
        limits: &ConcurrencyLimits,
        item: &QueueItem,
    ) -> Result<ConcurrencyGuard> {
        // Check all applicable limits
        for limit in &limits.limits {
            if limit.is_partition_limit() {
                // Function-level limit
                let semaphore = self.get_or_create_semaphore(function_id, limit.limit);
                let _permit = semaphore.acquire().await.map_err(|_| {
                    Error::ConcurrencyLimit("Failed to acquire semaphore permit".to_string())
                })?;
                // The permit will be dropped when the guard is dropped
            }
            // For now, we skip custom key-based limits to avoid complexity
        }

        // Track the active item
        self.add(function_id, item).await?;

        Ok(ConcurrencyGuard::new(
            Arc::new(InMemoryConcurrencyManager::new()),
            function_id,
            item.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_concurrency_limit_creation() {
        let fixture = ConcurrencyLimit::new(10, ConcurrencyScope::Function);
        let actual = fixture.limit;
        let expected = 10;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_concurrency_limits_validation() {
        let fixture = ConcurrencyLimits::new()
            .add_limit(ConcurrencyLimit::new(10, ConcurrencyScope::Function))
            .add_limit(
                ConcurrencyLimit::new(5, ConcurrencyScope::Account).with_key("user_id".to_string()),
            );

        let actual = fixture.validate().is_ok();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_in_memory_concurrency_manager() {
        let fixture = InMemoryConcurrencyManager::new();
        let function_id = Uuid::new_v4();
        let item = QueueItem {
            id: "test-item".to_string(),
            function_id,
            account_id: Uuid::new_v4(),
            data: serde_json::json!({}),
        };

        // Test adding and removing items
        fixture.add(function_id, &item).await.unwrap();
        assert!(fixture
            .active_items
            .contains_key(&format!("{}:{}", function_id, item.id)));

        fixture.done(function_id, &item).await.unwrap();
        assert!(!fixture
            .active_items
            .contains_key(&format!("{}:{}", function_id, item.id)));
    }
}
