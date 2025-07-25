//! Redis key generation utilities that maintain compatibility with the Go implementation.

use inngest_core::Identifier;
use ulid::Ulid;
use uuid::Uuid;

/// Generates Redis keys compatible with the Go implementation
#[derive(Debug, Clone)]
pub struct KeyGenerator {
    /// Base key prefix (e.g., "inngest")
    prefix: String,

    /// Whether to use sharded keys
    sharded: bool,
}

impl KeyGenerator {
    /// Create a new key generator
    pub fn new(prefix: impl Into<String>, sharded: bool) -> Self {
        Self {
            prefix: prefix.into(),
            sharded,
        }
    }

    /// Get the appropriate prefix for a run ID
    fn prefix_by_run_id(&self, run_id: Ulid) -> String {
        if self.sharded {
            format!("{}:{}", self.prefix, run_id)
        } else {
            self.prefix.clone()
        }
    }

    /// Get the appropriate prefix for an account ID
    fn prefix_by_account_id(&self, account_id: Uuid) -> String {
        if self.sharded {
            format!("{}:{}", self.prefix, account_id)
        } else {
            self.prefix.clone()
        }
    }

    /// Generate idempotency key - used for atomic lookup
    pub fn idempotency_key(&self, identifier: &Identifier) -> String {
        let prefix = self.prefix_by_account_id(identifier.account_id);
        format!("{{{}}}:key:{}", prefix, identifier.idempotency_key())
    }

    /// Generate run metadata key - stores run metadata like version, start time, etc.
    pub fn run_metadata_key(&self, run_id: Ulid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:metadata:{run_id}")
    }

    /// Generate event key - stores the specific event for a workflow run
    pub fn event_key(&self, workflow_id: Uuid, run_id: Ulid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:events:{workflow_id}:{run_id}")
    }

    /// Generate events key - stores batch events for a workflow run
    pub fn events_key(&self, workflow_id: Uuid, run_id: Ulid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:bulk-events:{workflow_id}:{run_id}")
    }

    /// Generate actions key - stores step response results
    pub fn actions_key(&self, workflow_id: Uuid, run_id: Ulid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:actions:{workflow_id}:{run_id}")
    }

    /// Generate errors key - stores step error information
    pub fn errors_key(&self, identifier: &Identifier) -> String {
        let prefix = self.prefix_by_run_id(identifier.run_id);
        format!(
            "{{{}}}:errors:{}:{}",
            prefix, identifier.workflow_id, identifier.run_id
        )
    }

    /// Generate history key - stores run history log entries
    pub fn history_key(&self, run_id: Ulid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:history:{run_id}")
    }

    /// Generate stack key - stores the execution stack
    pub fn stack_key(&self, run_id: Ulid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:stack:{run_id}")
    }

    /// Generate action inputs key - stores step input data
    pub fn action_inputs_key(&self, identifier: &Identifier) -> String {
        let prefix = self.prefix_by_run_id(identifier.run_id);
        format!(
            "{{{}}}:input:{}:{}",
            prefix, identifier.workflow_id, identifier.run_id
        )
    }

    /// Generate pending key - stores pending actions
    pub fn pending_key(&self, identifier: &Identifier) -> String {
        let prefix = self.prefix_by_run_id(identifier.run_id);
        format!(
            "{{{}}}:pending:{}:{}",
            prefix, identifier.workflow_id, identifier.run_id
        )
    }

    /// Generate pause consume key - idempotency key for pause consumption
    pub fn pause_consume_key(&self, run_id: Ulid, pause_id: Uuid) -> String {
        let prefix = self.prefix_by_run_id(run_id);
        format!("{{{prefix}}}:pause-consume:{run_id}:{pause_id}")
    }

    /// Generate pause key for storing pause data
    pub fn pause_key(&self, pause_id: Uuid) -> String {
        format!("{{{}}}:pause:{}", self.prefix, pause_id)
    }

    /// Generate pause event index key - for finding pauses by event name
    pub fn pause_event_index_key(&self, event_name: &str) -> String {
        format!("{{{}}}:pause-event:{}", self.prefix, event_name)
    }

    /// Generate pause timeout index key - for finding expired pauses
    pub fn pause_timeout_index_key(&self) -> String {
        format!("{{{}}}:pause-timeout", self.prefix)
    }
}

/// Key generator for different Redis backend types
pub trait RedisKeyGenerator {
    /// Generate all keys for a new state creation
    fn new_state_keys(&self, identifier: &Identifier) -> NewStateKeys;

    /// Generate keys for saving step response
    fn save_response_keys(&self, identifier: &Identifier) -> SaveResponseKeys;

    /// Generate keys for loading state
    fn load_state_keys(&self, identifier: &Identifier) -> LoadStateKeys;
}

/// Keys needed for new state creation
#[derive(Debug, Clone)]
pub struct NewStateKeys {
    pub events: String,
    pub metadata: String,
    pub actions: String,
    pub stack: String,
    pub action_inputs: String,
}

/// Keys needed for saving step response
#[derive(Debug, Clone)]
pub struct SaveResponseKeys {
    pub actions: String,
    pub metadata: String,
    pub stack: String,
    pub action_inputs: String,
    pub pending: String,
}

/// Keys needed for loading state
#[derive(Debug, Clone)]
pub struct LoadStateKeys {
    pub events: String,
    pub metadata: String,
    pub actions: String,
    pub stack: String,
    pub action_inputs: String,
    pub errors: String,
}

impl RedisKeyGenerator for KeyGenerator {
    fn new_state_keys(&self, identifier: &Identifier) -> NewStateKeys {
        NewStateKeys {
            events: self.events_key(identifier.workflow_id, identifier.run_id),
            metadata: self.run_metadata_key(identifier.run_id),
            actions: self.actions_key(identifier.workflow_id, identifier.run_id),
            stack: self.stack_key(identifier.run_id),
            action_inputs: self.action_inputs_key(identifier),
        }
    }

    fn save_response_keys(&self, identifier: &Identifier) -> SaveResponseKeys {
        SaveResponseKeys {
            actions: self.actions_key(identifier.workflow_id, identifier.run_id),
            metadata: self.run_metadata_key(identifier.run_id),
            stack: self.stack_key(identifier.run_id),
            action_inputs: self.action_inputs_key(identifier),
            pending: self.pending_key(identifier),
        }
    }

    fn load_state_keys(&self, identifier: &Identifier) -> LoadStateKeys {
        LoadStateKeys {
            events: self.events_key(identifier.workflow_id, identifier.run_id),
            metadata: self.run_metadata_key(identifier.run_id),
            actions: self.actions_key(identifier.workflow_id, identifier.run_id),
            stack: self.stack_key(identifier.run_id),
            action_inputs: self.action_inputs_key(identifier),
            errors: self.errors_key(identifier),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inngest_core::Identifier;
    use pretty_assertions::assert_eq;
    use ulid::Ulid;
    use uuid::Uuid;

    fn create_test_identifier() -> Identifier {
        Identifier::new(
            Ulid::new(),
            Uuid::new_v4(),
            0,
            Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
    }

    #[test]
    fn test_non_sharded_keys() {
        let generator = KeyGenerator::new("inngest", false);
        let identifier = create_test_identifier();

        // Test idempotency key
        let idempotency = generator.idempotency_key(&identifier);
        assert!(idempotency.starts_with("{inngest}:key:"));
        assert!(idempotency.contains(&identifier.idempotency_key()));

        // Test metadata key
        let metadata = generator.run_metadata_key(identifier.run_id);
        assert_eq!(
            metadata,
            format!("{{inngest}}:metadata:{}", identifier.run_id)
        );

        // Test events key
        let events = generator.events_key(identifier.workflow_id, identifier.run_id);
        assert_eq!(
            events,
            format!(
                "{{inngest}}:bulk-events:{}:{}",
                identifier.workflow_id, identifier.run_id
            )
        );
    }

    #[test]
    fn test_sharded_keys() {
        let generator = KeyGenerator::new("inngest", true);
        let identifier = create_test_identifier();

        // Test metadata key with sharding
        let metadata = generator.run_metadata_key(identifier.run_id);
        assert_eq!(
            metadata,
            format!(
                "{{inngest:{}}}:metadata:{}",
                identifier.run_id, identifier.run_id
            )
        );

        // Test idempotency key with account sharding
        let idempotency = generator.idempotency_key(&identifier);
        assert!(idempotency.starts_with(&format!("{{inngest:{}}}:key:", identifier.account_id)));
    }

    #[test]
    fn test_key_consistency() {
        let generator = KeyGenerator::new("test", false);
        let identifier = create_test_identifier();

        // Keys should be consistent across multiple calls
        let key1 = generator.actions_key(identifier.workflow_id, identifier.run_id);
        let key2 = generator.actions_key(identifier.workflow_id, identifier.run_id);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_key_uniqueness() {
        let generator = KeyGenerator::new("inngest", false);
        let id1 = create_test_identifier();
        let id2 = create_test_identifier();

        // Different identifiers should produce different keys
        let key1 = generator.actions_key(id1.workflow_id, id1.run_id);
        let key2 = generator.actions_key(id2.workflow_id, id2.run_id);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_redis_key_format() {
        let generator = KeyGenerator::new("inngest", false);
        let identifier = create_test_identifier();

        // All keys should follow the Redis cluster-compatible format
        let keys = vec![
            generator.idempotency_key(&identifier),
            generator.run_metadata_key(identifier.run_id),
            generator.events_key(identifier.workflow_id, identifier.run_id),
            generator.actions_key(identifier.workflow_id, identifier.run_id),
            generator.stack_key(identifier.run_id),
        ];

        for key in keys {
            // Should be wrapped in braces for Redis cluster compatibility
            assert!(key.starts_with('{'));
            assert!(key.contains('}'));
        }
    }

    #[test]
    fn test_key_generation_helper_methods() {
        let generator = KeyGenerator::new("inngest", false);
        let identifier = create_test_identifier();

        let new_keys = generator.new_state_keys(&identifier);
        assert!(new_keys.events.contains("bulk-events"));
        assert!(new_keys.metadata.contains("metadata"));
        assert!(new_keys.actions.contains("actions"));

        let save_keys = generator.save_response_keys(&identifier);
        assert!(save_keys.pending.contains("pending"));

        let load_keys = generator.load_state_keys(&identifier);
        assert!(load_keys.errors.contains("errors"));
    }

    #[test]
    fn test_pause_keys() {
        let generator = KeyGenerator::new("inngest", false);
        let pause_id = Uuid::new_v4();
        let run_id = Ulid::new();

        let pause_key = generator.pause_key(pause_id);
        assert_eq!(pause_key, format!("{{inngest}}:pause:{}", pause_id));

        let consume_key = generator.pause_consume_key(run_id, pause_id);
        assert_eq!(
            consume_key,
            format!("{{inngest}}:pause-consume:{}:{}", run_id, pause_id)
        );

        let event_index = generator.pause_event_index_key("user.created");
        assert_eq!(event_index, "{inngest}:pause-event:user.created");
    }
}
