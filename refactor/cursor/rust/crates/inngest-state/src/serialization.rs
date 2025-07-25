//! Serialization utilities for state data.
//!
//! This module provides efficient serialization and deserialization
//! of state data for Redis storage.

use crate::error::{StateError, StateResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument};

/// Serialization format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerializationFormat {
    /// JSON format (human-readable, larger size)
    #[default]
    Json,
    /// Binary format (compact, faster)
    Binary,
}

/// Serialization helper for state data
#[derive(Debug, Clone)]
pub struct Serializer {
    format: SerializationFormat,
    compress: bool,
}

impl Serializer {
    /// Create a new serializer with default JSON format
    pub fn new() -> Self {
        Self {
            format: SerializationFormat::Json,
            compress: false,
        }
    }

    /// Create a new serializer with specified format
    pub fn with_format(format: SerializationFormat) -> Self {
        Self {
            format,
            compress: false,
        }
    }

    /// Enable compression for serialized data
    pub fn with_compression(mut self, compress: bool) -> Self {
        self.compress = compress;
        self
    }

    /// Serialize data to bytes
    #[instrument(skip(self, data))]
    pub fn serialize<T>(&self, data: &T) -> StateResult<Vec<u8>>
    where
        T: Serialize + std::fmt::Debug,
    {
        debug!("Serializing data with format {:?}", self.format);

        let bytes = match self.format {
            SerializationFormat::Json => serde_json::to_vec(data).map_err(StateError::from)?,
            SerializationFormat::Binary => {
                // For now, use JSON as binary format placeholder
                // In production, you might use bincode, protobuf, etc.
                serde_json::to_vec(data).map_err(StateError::from)?
            }
        };

        if self.compress {
            // Placeholder for compression
            // In production, you might use zstd, lz4, etc.
            debug!("Compression enabled but not implemented");
        }

        debug!("Serialized {} bytes", bytes.len());
        Ok(bytes)
    }

    /// Deserialize data from bytes
    #[instrument(skip(self, bytes))]
    pub fn deserialize<T>(&self, bytes: &[u8]) -> StateResult<T>
    where
        T: for<'de> Deserialize<'de> + std::fmt::Debug,
    {
        debug!(
            "Deserializing {} bytes with format {:?}",
            bytes.len(),
            self.format
        );

        let decompressed_bytes = if self.compress {
            // Placeholder for decompression
            debug!("Decompression enabled but not implemented");
            bytes
        } else {
            bytes
        };

        let data = match self.format {
            SerializationFormat::Json => {
                serde_json::from_slice(decompressed_bytes).map_err(StateError::from)?
            }
            SerializationFormat::Binary => {
                // For now, use JSON as binary format placeholder
                serde_json::from_slice(decompressed_bytes).map_err(StateError::from)?
            }
        };

        debug!("Successfully deserialized data");
        Ok(data)
    }

    /// Serialize to string (for JSON format)
    pub fn serialize_to_string<T>(&self, data: &T) -> StateResult<String>
    where
        T: Serialize,
    {
        match self.format {
            SerializationFormat::Json => serde_json::to_string(data).map_err(StateError::from),
            SerializationFormat::Binary => Err(StateError::Internal(
                "Cannot serialize binary format to string".to_string(),
            )),
        }
    }

    /// Deserialize from string (for JSON format)
    pub fn deserialize_from_string<T>(&self, data: &str) -> StateResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        match self.format {
            SerializationFormat::Json => serde_json::from_str(data).map_err(StateError::from),
            SerializationFormat::Binary => Err(StateError::Internal(
                "Cannot deserialize binary format from string".to_string(),
            )),
        }
    }
}

impl Default for Serializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Step result data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepResult {
    pub step_id: String,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// Run state data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunState {
    pub run_id: String,
    pub function_id: String,
    pub status: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub event_data: serde_json::Value,
    pub metadata: HashMap<String, serde_json::Value>,
    pub steps: HashMap<String, StepResult>,
}

/// Pause state data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PauseState {
    pub pause_id: String,
    pub run_id: String,
    pub step_id: String,
    pub event: String,
    pub timeout: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_serializer_creation() {
        let serializer = Serializer::new();
        assert_eq!(serializer.format, SerializationFormat::Json);
        assert!(!serializer.compress);

        let binary_serializer = Serializer::with_format(SerializationFormat::Binary);
        assert_eq!(binary_serializer.format, SerializationFormat::Binary);

        let compressed_serializer = Serializer::new().with_compression(true);
        assert!(compressed_serializer.compress);
    }

    #[test]
    fn test_json_serialization() {
        let serializer = Serializer::new();
        let data = json!({
            "key": "value",
            "number": 42,
            "nested": {
                "inner": "data"
            }
        });

        // Test serialize/deserialize cycle
        let bytes = serializer.serialize(&data).unwrap();
        let deserialized: serde_json::Value = serializer.deserialize(&bytes).unwrap();
        assert_eq!(data, deserialized);

        // Test string serialization
        let string_data = serializer.serialize_to_string(&data).unwrap();
        let from_string: serde_json::Value =
            serializer.deserialize_from_string(&string_data).unwrap();
        assert_eq!(data, from_string);
    }

    #[test]
    fn test_step_result_serialization() {
        let serializer = Serializer::new();
        let step_result = StepResult {
            step_id: "step-1".to_string(),
            output: json!({"result": "success"}),
            error: None,
            completed_at: chrono::Utc::now(),
        };

        let bytes = serializer.serialize(&step_result).unwrap();
        let deserialized: StepResult = serializer.deserialize(&bytes).unwrap();
        assert_eq!(step_result, deserialized);
    }

    #[test]
    fn test_run_state_serialization() {
        let serializer = Serializer::new();
        let now = chrono::Utc::now();

        let mut steps = HashMap::new();
        steps.insert(
            "step-1".to_string(),
            StepResult {
                step_id: "step-1".to_string(),
                output: json!({"result": "success"}),
                error: None,
                completed_at: now,
            },
        );

        let run_state = RunState {
            run_id: "run-123".to_string(),
            function_id: "fn-456".to_string(),
            status: "running".to_string(),
            started_at: now,
            updated_at: now,
            event_data: json!({"user_id": "789"}),
            metadata: HashMap::new(),
            steps,
        };

        let bytes = serializer.serialize(&run_state).unwrap();
        let deserialized: RunState = serializer.deserialize(&bytes).unwrap();
        assert_eq!(run_state, deserialized);
    }

    #[test]
    fn test_pause_state_serialization() {
        let serializer = Serializer::new();
        let now = chrono::Utc::now();

        let pause_state = PauseState {
            pause_id: "pause-123".to_string(),
            run_id: "run-456".to_string(),
            step_id: "step-789".to_string(),
            event: "user.action".to_string(),
            timeout: Some(now + chrono::Duration::hours(1)),
            created_at: now,
        };

        let bytes = serializer.serialize(&pause_state).unwrap();
        let deserialized: PauseState = serializer.deserialize(&bytes).unwrap();
        assert_eq!(pause_state, deserialized);
    }

    #[test]
    fn test_binary_format_string_operations() {
        let serializer = Serializer::with_format(SerializationFormat::Binary);
        let data = json!({"test": "data"});

        // Binary format should not support string operations
        assert!(serializer.serialize_to_string(&data).is_err());
        assert!(
            serializer
                .deserialize_from_string::<serde_json::Value>("{}")
                .is_err()
        );
    }

    #[test]
    fn test_serialization_format_default() {
        assert_eq!(SerializationFormat::default(), SerializationFormat::Json);
    }
}
