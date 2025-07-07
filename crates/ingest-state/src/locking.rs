//! Optimistic locking implementation for concurrency control
//!
//! This module provides optimistic locking mechanisms to prevent
//! race conditions when multiple processes update the same state.

use crate::{Result, StateError};
use ingest_core::Id;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Manages optimistic locking for state updates
pub struct OptimisticLockManager {
    /// In-memory lock tracking
    locks: Arc<RwLock<HashMap<Id, LockInfo>>>,
    /// Lock timeout duration
    lock_timeout: Duration,
}

#[derive(Debug, Clone)]
struct LockInfo {
    version: u64,
    last_updated: Instant,
    holder: Option<String>, // Optional lock holder identifier
}

impl OptimisticLockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            lock_timeout: Duration::from_secs(30), // 30 second timeout
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            lock_timeout: timeout,
        }
    }

    /// Check if the version is valid for update
    pub async fn check_version(&self, run_id: &Id, expected_version: u64) -> Result<()> {
        let locks = self.locks.read().await;

        if let Some(lock_info) = locks.get(run_id) {
            // Check if lock has expired
            if lock_info.last_updated.elapsed() > self.lock_timeout {
                drop(locks);
                self.cleanup_expired_lock(run_id).await;
                return Ok(()); // Expired lock, allow operation
            }

            // Check version
            if lock_info.version != expected_version {
                return Err(StateError::optimistic_lock_failure(format!(
                    "Version mismatch: expected {}, got {}",
                    expected_version, lock_info.version
                )));
            }
        }

        Ok(())
    }

    /// Acquire a lock for a state update
    pub async fn acquire_lock(
        &self,
        run_id: &Id,
        version: u64,
        holder: Option<String>,
    ) -> Result<()> {
        let mut locks = self.locks.write().await;

        // Check if there's an existing lock
        if let Some(existing) = locks.get(run_id) {
            // Check if lock has expired
            if existing.last_updated.elapsed() <= self.lock_timeout {
                return Err(StateError::optimistic_lock_failure(format!(
                    "State {run_id} is locked by another process"
                )));
            }
        }

        // Acquire the lock
        locks.insert(
            run_id.clone(),
            LockInfo {
                version,
                last_updated: Instant::now(),
                holder,
            },
        );

        Ok(())
    }

    /// Release a lock after successful update
    pub async fn release_lock(&self, run_id: &Id, _new_version: u64) -> Result<()> {
        let mut locks = self.locks.write().await;

        // Remove the lock to allow new acquisitions
        locks.remove(run_id);

        Ok(())
    }

    /// Force release a lock (use with caution)
    pub async fn force_release_lock(&self, run_id: &Id) -> Result<()> {
        let mut locks = self.locks.write().await;
        locks.remove(run_id);
        Ok(())
    }

    /// Get current lock info
    pub async fn get_lock_info(&self, run_id: &Id) -> Option<(u64, Duration, Option<String>)> {
        let locks = self.locks.read().await;

        if let Some(lock_info) = locks.get(run_id) {
            let age = lock_info.last_updated.elapsed();
            Some((lock_info.version, age, lock_info.holder.clone()))
        } else {
            None
        }
    }

    /// Check if a state is currently locked
    pub async fn is_locked(&self, run_id: &Id) -> bool {
        let locks = self.locks.read().await;

        if let Some(lock_info) = locks.get(run_id) {
            lock_info.last_updated.elapsed() <= self.lock_timeout
        } else {
            false
        }
    }

    /// Cleanup expired locks
    pub async fn cleanup_expired_locks(&self) -> usize {
        let mut locks = self.locks.write().await;
        let initial_count = locks.len();

        locks.retain(|_, lock_info| lock_info.last_updated.elapsed() <= self.lock_timeout);

        initial_count - locks.len()
    }

    /// Cleanup a specific expired lock
    async fn cleanup_expired_lock(&self, run_id: &Id) {
        let mut locks = self.locks.write().await;

        if let Some(lock_info) = locks.get(run_id) {
            if lock_info.last_updated.elapsed() > self.lock_timeout {
                locks.remove(run_id);
            }
        }
    }

    /// Get all current locks (for debugging)
    pub async fn get_all_locks(&self) -> HashMap<Id, (u64, Duration, Option<String>)> {
        let locks = self.locks.read().await;

        locks
            .iter()
            .map(|(id, lock_info)| {
                (
                    id.clone(),
                    (
                        lock_info.version,
                        lock_info.last_updated.elapsed(),
                        lock_info.holder.clone(),
                    ),
                )
            })
            .collect()
    }

    /// Get lock statistics
    pub async fn get_lock_stats(&self) -> LockStats {
        let locks = self.locks.read().await;
        let now = Instant::now();

        let total_locks = locks.len();
        let active_locks = locks
            .values()
            .filter(|lock| now.duration_since(lock.last_updated) <= self.lock_timeout)
            .count();
        let expired_locks = total_locks - active_locks;

        LockStats {
            total_locks,
            active_locks,
            expired_locks,
            timeout_duration: self.lock_timeout,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LockStats {
    pub total_locks: usize,
    pub active_locks: usize,
    pub expired_locks: usize,
    pub timeout_duration: Duration,
}

impl Default for OptimisticLockManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_lock_acquisition_and_release() {
        let manager = OptimisticLockManager::new();
        let run_id = generate_id_with_prefix("run");

        // Should be able to acquire lock initially
        assert!(
            manager
                .acquire_lock(&run_id, 1, Some("test".to_string()))
                .await
                .is_ok()
        );

        // Should not be able to acquire lock again
        assert!(
            manager
                .acquire_lock(&run_id, 1, Some("test2".to_string()))
                .await
                .is_err()
        );

        // Should be able to release lock
        assert!(manager.release_lock(&run_id, 2).await.is_ok());

        // Should be able to acquire lock again after release
        assert!(
            manager
                .acquire_lock(&run_id, 2, Some("test3".to_string()))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_version_checking() {
        let manager = OptimisticLockManager::new();
        let run_id = generate_id_with_prefix("run");

        // Acquire lock with version 1
        manager.acquire_lock(&run_id, 1, None).await.unwrap();

        // Check with correct version should succeed
        assert!(manager.check_version(&run_id, 1).await.is_ok());

        // Check with incorrect version should fail
        assert!(manager.check_version(&run_id, 2).await.is_err());

        // Release lock (removes it completely)
        manager.release_lock(&run_id, 2).await.unwrap();

        // After release, any version check should succeed (no lock exists)
        assert!(manager.check_version(&run_id, 1).await.is_ok());
        assert!(manager.check_version(&run_id, 2).await.is_ok());
    }

    #[tokio::test]
    async fn test_lock_expiration() {
        let manager = OptimisticLockManager::with_timeout(Duration::from_millis(100));
        let run_id = generate_id_with_prefix("run");

        // Acquire lock
        manager.acquire_lock(&run_id, 1, None).await.unwrap();

        // Should be locked initially
        assert!(manager.is_locked(&run_id).await);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should not be locked after expiration
        assert!(!manager.is_locked(&run_id).await);

        // Should be able to acquire lock again
        assert!(manager.acquire_lock(&run_id, 1, None).await.is_ok());
    }

    #[tokio::test]
    async fn test_lock_cleanup() {
        let manager = OptimisticLockManager::with_timeout(Duration::from_millis(100));

        // Acquire multiple locks
        for i in 0..5 {
            let run_id = generate_id_with_prefix(&format!("run_{i}"));
            manager.acquire_lock(&run_id, 1, None).await.unwrap();
        }

        // All should be active initially
        let stats = manager.get_lock_stats().await;
        assert_eq!(stats.active_locks, 5);
        assert_eq!(stats.expired_locks, 0);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Clean up expired locks
        let cleaned = manager.cleanup_expired_locks().await;
        assert_eq!(cleaned, 5);

        // All should be cleaned up
        let stats = manager.get_lock_stats().await;
        assert_eq!(stats.total_locks, 0);
    }

    #[tokio::test]
    async fn test_lock_info() {
        let manager = OptimisticLockManager::new();
        let run_id = generate_id_with_prefix("run");
        let holder = "test_holder".to_string();

        // No lock initially
        assert!(manager.get_lock_info(&run_id).await.is_none());

        // Acquire lock
        manager
            .acquire_lock(&run_id, 5, Some(holder.clone()))
            .await
            .unwrap();

        // Should have lock info
        let info = manager.get_lock_info(&run_id).await.unwrap();
        assert_eq!(info.0, 5); // version
        assert!(info.1 < Duration::from_secs(1)); // age should be small
        assert_eq!(info.2, Some(holder)); // holder
    }
}
