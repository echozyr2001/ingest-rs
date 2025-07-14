//! Data retention policies for historical audit data
//!
//! This module manages retention policies for audit data, including
//! automatic cleanup, policy configuration, and compliance features.

use crate::error::Result;
use chrono::{DateTime, Utc};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Retention policy configuration
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct RetentionPolicy {
    /// Unique identifier for the policy
    pub id: Uuid,
    /// Human-readable name for the policy
    pub name: String,
    /// Description of the policy
    pub description: Option<String>,
    /// How long to keep data in the primary storage (in days)
    pub primary_retention_days: u32,
    /// How long to keep data in archive storage (in days)
    pub archive_retention_days: u32,
    /// Resource types this policy applies to
    pub resource_types: Vec<crate::audit::ResourceType>,
    /// Event types this policy applies to
    pub event_types: Option<Vec<crate::audit::AuditEventType>>,
    /// Whether this policy is active
    pub enabled: bool,
    /// Priority of this policy (higher number = higher priority)
    pub priority: u32,
    /// Additional metadata for the policy
    pub metadata: HashMap<String, String>,
    /// When this policy was created
    pub created_at: DateTime<Utc>,
    /// When this policy was last updated
    pub updated_at: DateTime<Utc>,
}

impl RetentionPolicy {
    /// Create a new retention policy
    pub fn new(
        name: impl Into<String>,
        primary_retention_days: u32,
        archive_retention_days: u32,
        resource_types: Vec<crate::audit::ResourceType>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            primary_retention_days,
            archive_retention_days,
            resource_types,
            event_types: None,
            enabled: true,
            priority: 100,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if this policy applies to a given resource type and event type
    pub fn applies_to(
        &self,
        resource_type: &crate::audit::ResourceType,
        event_type: &crate::audit::AuditEventType,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        if !self.resource_types.contains(resource_type) {
            return false;
        }

        if let Some(ref event_types) = self.event_types {
            if !event_types.contains(event_type) {
                return false;
            }
        }

        true
    }

    /// Get the total retention period (primary + archive)
    pub fn total_retention_days(&self) -> u32 {
        self.primary_retention_days + self.archive_retention_days
    }

    /// Check if data should be archived based on this policy
    pub fn should_archive(&self, event_date: DateTime<Utc>) -> bool {
        let age_days = (Utc::now() - event_date).num_days();
        age_days >= self.primary_retention_days as i64
    }

    /// Check if data should be deleted based on this policy
    pub fn should_delete(&self, event_date: DateTime<Utc>) -> bool {
        let age_days = (Utc::now() - event_date).num_days();
        age_days >= self.total_retention_days() as i64
    }
}

/// Default retention policies for different resource types
pub struct DefaultPolicies;

impl DefaultPolicies {
    /// Get default policy for function-related events
    pub fn function_policy() -> RetentionPolicy {
        RetentionPolicy::new(
            "Function Events",
            90,  // 90 days in primary storage
            365, // 1 year in archive
            vec![crate::audit::ResourceType::Function],
        )
        .description("Default retention policy for function-related audit events")
    }

    /// Get default policy for event-related events
    pub fn event_policy() -> RetentionPolicy {
        RetentionPolicy::new(
            "Event Data",
            30, // 30 days in primary storage
            90, // 90 days in archive
            vec![crate::audit::ResourceType::Event],
        )
        .description("Default retention policy for event data")
    }

    /// Get default policy for user-related events
    pub fn user_policy() -> RetentionPolicy {
        RetentionPolicy::new(
            "User Events",
            180,  // 180 days in primary storage
            1095, // 3 years in archive
            vec![crate::audit::ResourceType::User],
        )
        .description("Default retention policy for user-related audit events")
    }

    /// Get default policy for system events
    pub fn system_policy() -> RetentionPolicy {
        RetentionPolicy::new(
            "System Events",
            365,  // 1 year in primary storage
            1095, // 3 years in archive
            vec![crate::audit::ResourceType::System],
        )
        .description("Default retention policy for system events")
    }

    /// Get all default policies
    pub fn all() -> Vec<RetentionPolicy> {
        vec![
            Self::function_policy(),
            Self::event_policy(),
            Self::user_policy(),
            Self::system_policy(),
        ]
    }
}

/// Retention statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionStats {
    /// Total number of events in primary storage
    pub primary_events: u64,
    /// Total number of events in archive storage
    pub archived_events: u64,
    /// Number of events eligible for archiving
    pub archival_eligible: u64,
    /// Number of events eligible for deletion
    pub deletion_eligible: u64,
    /// Total storage size in bytes
    pub total_size_bytes: u64,
    /// Archive storage size in bytes
    pub archive_size_bytes: u64,
}

/// Retention manager for handling data lifecycle
pub struct RetentionManager {
    #[allow(dead_code)]
    storage: Arc<
        dyn ingest_storage::Storage<Transaction = ingest_storage::postgres::PostgresTransaction>,
    >,
    policies: Vec<RetentionPolicy>,
}

impl RetentionManager {
    /// Create a new retention manager
    pub async fn new(
        storage: Arc<
            dyn ingest_storage::Storage<Transaction = ingest_storage::postgres::PostgresTransaction>,
        >,
    ) -> Result<Self> {
        let policies = DefaultPolicies::all();
        Ok(Self { storage, policies })
    }

    /// Create retention manager with custom policies
    pub async fn with_policies(
        storage: Arc<
            dyn ingest_storage::Storage<Transaction = ingest_storage::postgres::PostgresTransaction>,
        >,
        policies: Vec<RetentionPolicy>,
    ) -> Result<Self> {
        Ok(Self { storage, policies })
    }

    /// Add a retention policy
    pub fn add_policy(&mut self, policy: RetentionPolicy) {
        self.policies.push(policy);
        // Sort by priority (highest first)
        self.policies.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove a retention policy by ID
    pub fn remove_policy(&mut self, policy_id: Uuid) -> bool {
        let initial_len = self.policies.len();
        self.policies.retain(|p| p.id != policy_id);
        self.policies.len() != initial_len
    }

    /// Get the applicable policy for a resource and event type
    pub fn get_applicable_policy(
        &self,
        resource_type: &crate::audit::ResourceType,
        event_type: &crate::audit::AuditEventType,
    ) -> Option<&RetentionPolicy> {
        self.policies
            .iter()
            .find(|policy| policy.applies_to(resource_type, event_type))
    }

    /// Run retention cleanup process
    pub async fn run_cleanup(&self) -> Result<RetentionStats> {
        info!("Starting retention cleanup process");

        let stats = RetentionStats {
            primary_events: 0,
            archived_events: 0,
            archival_eligible: 0,
            deletion_eligible: 0,
            total_size_bytes: 0,
            archive_size_bytes: 0,
        };

        // For each policy, find and process applicable events
        for policy in &self.policies {
            if !policy.enabled {
                continue;
            }

            debug!("Processing retention policy: {}", policy.name);

            // This would query events and apply retention rules
            // For now, just log the policy being processed
            info!(
                "Policy '{}' - Primary: {} days, Archive: {} days",
                policy.name, policy.primary_retention_days, policy.archive_retention_days
            );
        }

        info!("Retention cleanup completed");
        Ok(stats)
    }

    /// Get retention statistics
    pub async fn get_stats(&self) -> Result<RetentionStats> {
        // This would query the database for actual statistics
        Ok(RetentionStats {
            primary_events: 0,
            archived_events: 0,
            archival_eligible: 0,
            deletion_eligible: 0,
            total_size_bytes: 0,
            archive_size_bytes: 0,
        })
    }

    /// Preview what would be affected by retention policies
    pub async fn preview_cleanup(&self) -> Result<HashMap<String, u64>> {
        let mut preview = HashMap::new();

        for policy in &self.policies {
            if !policy.enabled {
                continue;
            }

            // This would calculate what would be affected
            preview.insert(format!("policy_{}_archival", policy.name), 0);
            preview.insert(format!("policy_{}_deletion", policy.name), 0);
        }

        Ok(preview)
    }

    /// Validate retention policies for conflicts
    pub fn validate_policies(&self) -> Result<()> {
        // Check for conflicting policies
        for (i, policy1) in self.policies.iter().enumerate() {
            for policy2 in self.policies.iter().skip(i + 1) {
                if policy1.priority == policy2.priority {
                    // Check if they overlap in resource/event types
                    let overlaps = policy1
                        .resource_types
                        .iter()
                        .any(|rt| policy2.resource_types.contains(rt));

                    if overlaps {
                        warn!(
                            "Policies '{}' and '{}' have same priority and overlapping resource types",
                            policy1.name, policy2.name
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_retention_policy_creation() {
        let fixture = RetentionPolicy::new(
            "Test Policy",
            30,
            90,
            vec![crate::audit::ResourceType::Function],
        );

        assert_eq!(fixture.name, "Test Policy");
        assert_eq!(fixture.primary_retention_days, 30);
        assert_eq!(fixture.archive_retention_days, 90);
        assert_eq!(fixture.total_retention_days(), 120);
        assert!(fixture.enabled);
    }

    #[test]
    fn test_retention_policy_setters() {
        let fixture =
            RetentionPolicy::new("Test", 30, 90, vec![crate::audit::ResourceType::Function])
                .description("Test description")
                .enabled(false)
                .priority(200u32);

        assert_eq!(fixture.description, Some("Test description".to_string()));
        assert!(!fixture.enabled);
        assert_eq!(fixture.priority, 200);
    }

    #[test]
    fn test_retention_policy_applies_to() {
        let fixture =
            RetentionPolicy::new("Test", 30, 90, vec![crate::audit::ResourceType::Function]);

        assert!(fixture.applies_to(
            &crate::audit::ResourceType::Function,
            &crate::audit::AuditEventType::Created
        ));

        assert!(!fixture.applies_to(
            &crate::audit::ResourceType::Event,
            &crate::audit::AuditEventType::Created
        ));
    }

    #[test]
    fn test_retention_policy_should_archive() {
        let fixture =
            RetentionPolicy::new("Test", 30, 90, vec![crate::audit::ResourceType::Function]);

        let old_date = Utc::now() - Duration::days(35);
        let recent_date = Utc::now() - Duration::days(15);

        assert!(fixture.should_archive(old_date));
        assert!(!fixture.should_archive(recent_date));
    }

    #[test]
    fn test_retention_policy_should_delete() {
        let fixture =
            RetentionPolicy::new("Test", 30, 90, vec![crate::audit::ResourceType::Function]);

        let very_old_date = Utc::now() - Duration::days(125);
        let old_date = Utc::now() - Duration::days(35);

        assert!(fixture.should_delete(very_old_date));
        assert!(!fixture.should_delete(old_date));
    }

    #[test]
    fn test_default_policies() {
        let fixture = DefaultPolicies::all();
        assert_eq!(fixture.len(), 4);

        let function_policy = &fixture[0];
        assert_eq!(function_policy.name, "Function Events");
        assert_eq!(function_policy.primary_retention_days, 90);
    }

    #[test]
    fn test_retention_stats_creation() {
        let fixture = RetentionStats {
            primary_events: 1000,
            archived_events: 500,
            archival_eligible: 100,
            deletion_eligible: 50,
            total_size_bytes: 1024 * 1024,
            archive_size_bytes: 512 * 1024,
        };

        assert_eq!(fixture.primary_events, 1000);
        assert_eq!(fixture.archived_events, 500);
        assert_eq!(fixture.total_size_bytes, 1024 * 1024);
    }

    #[test]
    fn test_retention_policy_serialization() {
        let fixture =
            RetentionPolicy::new("Test", 30, 90, vec![crate::audit::ResourceType::Function]);

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }
}
