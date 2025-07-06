//! Event serialization and format handling

use crate::{Result, error::EventError};
use ingest_core::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// Serialization format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializationFormat {
    /// JSON format
    Json,
    /// MessagePack binary format
    MessagePack,
    /// Compact JSON (no pretty printing)
    JsonCompact,
}

impl SerializationFormat {
    /// Get format as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::MessagePack => "messagepack",
            Self::JsonCompact => "json-compact",
        }
    }

    /// Get content type for HTTP
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Json | Self::JsonCompact => "application/json",
            Self::MessagePack => "application/msgpack",
        }
    }
}

impl FromStr for SerializationFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "messagepack" | "msgpack" => Ok(Self::MessagePack),
            "json-compact" | "compact" => Ok(Self::JsonCompact),
            _ => Err(format!("Unknown serialization format: {s}")),
        }
    }
}

impl Default for SerializationFormat {
    fn default() -> Self {
        Self::Json
    }
}

/// Event serializer
#[derive(Debug)]
pub struct EventSerializer {
    /// Default format
    default_format: SerializationFormat,
    /// Format-specific configurations
    configs: HashMap<SerializationFormat, SerializationConfig>,
}

/// Serialization configuration
#[derive(Debug, Clone)]
pub struct SerializationConfig {
    /// Pretty print JSON
    pub pretty: bool,
    /// Compress output
    pub compress: bool,
    /// Include metadata
    pub include_metadata: bool,
    /// Maximum size in bytes
    pub max_size: Option<usize>,
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            pretty: true,
            compress: false,
            include_metadata: true,
            max_size: None,
        }
    }
}

impl EventSerializer {
    /// Create new event serializer
    pub fn new() -> Self {
        Self {
            default_format: SerializationFormat::default(),
            configs: HashMap::new(),
        }
    }

    /// Set default format
    pub fn with_default_format(mut self, format: SerializationFormat) -> Self {
        self.default_format = format;
        self
    }

    /// Set configuration for a format
    pub fn with_config(mut self, format: SerializationFormat, config: SerializationConfig) -> Self {
        self.configs.insert(format, config);
        self
    }

    /// Serialize event to bytes
    pub fn serialize(&self, event: &Event, format: Option<SerializationFormat>) -> Result<Vec<u8>> {
        let format = format.unwrap_or(self.default_format);
        let config = self.configs.get(&format).cloned().unwrap_or_default();

        // Check size limit before serialization
        if let Some(max_size) = config.max_size {
            let estimated_size = self.estimate_size(event)?;
            if estimated_size > max_size {
                return Err(EventError::validation(format!(
                    "Event size {estimated_size} exceeds maximum {max_size}"
                )));
            }
        }

        let bytes = match format {
            SerializationFormat::Json => {
                if config.pretty {
                    serde_json::to_vec_pretty(event)?
                } else {
                    serde_json::to_vec(event)?
                }
            }
            SerializationFormat::JsonCompact => serde_json::to_vec(event)?,
            SerializationFormat::MessagePack => rmp_serde::to_vec(event).map_err(|e| {
                EventError::validation(format!("MessagePack serialization failed: {e}"))
            })?,
        };

        // Validate size after serialization
        if let Some(max_size) = config.max_size {
            if bytes.len() > max_size {
                return Err(EventError::validation(format!(
                    "Serialized event size {} exceeds maximum {}",
                    bytes.len(),
                    max_size
                )));
            }
        }

        Ok(bytes)
    }

    /// Deserialize event from bytes
    pub fn deserialize(&self, bytes: &[u8], format: Option<SerializationFormat>) -> Result<Event> {
        let format = format.unwrap_or(self.default_format);

        let event = match format {
            SerializationFormat::Json | SerializationFormat::JsonCompact => {
                serde_json::from_slice(bytes)?
            }
            SerializationFormat::MessagePack => rmp_serde::from_slice(bytes).map_err(|e| {
                EventError::validation(format!("MessagePack deserialization failed: {e}"))
            })?,
        };

        Ok(event)
    }

    /// Serialize event to string (for text formats)
    pub fn serialize_to_string(
        &self,
        event: &Event,
        format: Option<SerializationFormat>,
    ) -> Result<String> {
        let format = format.unwrap_or(self.default_format);

        match format {
            SerializationFormat::Json => {
                let config = self.configs.get(&format).cloned().unwrap_or_default();
                if config.pretty {
                    Ok(serde_json::to_string_pretty(event)?)
                } else {
                    Ok(serde_json::to_string(event)?)
                }
            }
            SerializationFormat::JsonCompact => Ok(serde_json::to_string(event)?),
            SerializationFormat::MessagePack => Err(EventError::validation(
                "MessagePack format is binary, use serialize() instead",
            )),
        }
    }

    /// Deserialize event from string (for text formats)
    pub fn deserialize_from_string(
        &self,
        data: &str,
        format: Option<SerializationFormat>,
    ) -> Result<Event> {
        let format = format.unwrap_or(self.default_format);

        match format {
            SerializationFormat::Json | SerializationFormat::JsonCompact => {
                Ok(serde_json::from_str(data)?)
            }
            SerializationFormat::MessagePack => Err(EventError::validation(
                "MessagePack format is binary, use deserialize() instead",
            )),
        }
    }

    /// Estimate serialized size
    fn estimate_size(&self, event: &Event) -> Result<usize> {
        // Quick estimation using JSON serialization
        let json_bytes = serde_json::to_vec(event)?;
        Ok(json_bytes.len())
    }

    /// Get supported formats
    pub fn supported_formats() -> Vec<SerializationFormat> {
        vec![
            SerializationFormat::Json,
            SerializationFormat::JsonCompact,
            SerializationFormat::MessagePack,
        ]
    }
}

impl Default for EventSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialized event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedEvent {
    /// Serialized data
    pub data: Vec<u8>,
    /// Serialization format
    pub format: String,
    /// Content type
    pub content_type: String,
    /// Size in bytes
    pub size: usize,
    /// Compression used
    pub compressed: bool,
}

impl SerializedEvent {
    /// Create new serialized event
    pub fn new(data: Vec<u8>, format: SerializationFormat) -> Self {
        Self {
            size: data.len(),
            content_type: format.content_type().to_string(),
            format: format.as_str().to_string(),
            data,
            compressed: false,
        }
    }

    /// Mark as compressed
    pub fn with_compression(mut self) -> Self {
        self.compressed = true;
        self
    }

    /// Get format
    pub fn format(&self) -> Option<SerializationFormat> {
        self.format.parse().ok()
    }
}

/// Create a JSON serializer with pretty printing
pub fn create_json_serializer() -> EventSerializer {
    let config = SerializationConfig {
        pretty: true,
        compress: false,
        include_metadata: true,
        max_size: None,
    };

    EventSerializer::new()
        .with_default_format(SerializationFormat::Json)
        .with_config(SerializationFormat::Json, config)
}

/// Create a compact serializer for high-throughput scenarios
pub fn create_compact_serializer() -> EventSerializer {
    let config = SerializationConfig {
        pretty: false,
        compress: true,
        include_metadata: false,
        max_size: Some(1024 * 1024), // 1MB limit
    };

    EventSerializer::new()
        .with_default_format(SerializationFormat::JsonCompact)
        .with_config(SerializationFormat::JsonCompact, config)
}

/// Create a binary serializer using MessagePack
pub fn create_binary_serializer() -> EventSerializer {
    let config = SerializationConfig {
        pretty: false,
        compress: false,
        include_metadata: true,
        max_size: Some(10 * 1024 * 1024), // 10MB limit
    };

    EventSerializer::new()
        .with_default_format(SerializationFormat::MessagePack)
        .with_config(SerializationFormat::MessagePack, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event() -> Event {
        Event::new("user.login", json!({"user_id": "123", "action": "login"}))
    }

    #[test]
    fn test_serialization_format_from_str() {
        assert_eq!(
            "json".parse::<SerializationFormat>(),
            Ok(SerializationFormat::Json)
        );
        assert_eq!(
            "messagepack".parse::<SerializationFormat>(),
            Ok(SerializationFormat::MessagePack)
        );
        assert_eq!(
            "compact".parse::<SerializationFormat>(),
            Ok(SerializationFormat::JsonCompact)
        );
        assert!("invalid".parse::<SerializationFormat>().is_err());
    }

    #[test]
    fn test_serialization_format_as_str() {
        assert_eq!(SerializationFormat::Json.as_str(), "json");
        assert_eq!(SerializationFormat::MessagePack.as_str(), "messagepack");
        assert_eq!(SerializationFormat::JsonCompact.as_str(), "json-compact");
    }

    #[test]
    fn test_serialization_format_content_type() {
        assert_eq!(SerializationFormat::Json.content_type(), "application/json");
        assert_eq!(
            SerializationFormat::MessagePack.content_type(),
            "application/msgpack"
        );
        assert_eq!(
            SerializationFormat::JsonCompact.content_type(),
            "application/json"
        );
    }

    #[test]
    fn test_event_serializer_creation() {
        let fixture = EventSerializer::new();
        assert_eq!(fixture.default_format, SerializationFormat::Json);
        assert_eq!(fixture.configs.len(), 0);
    }

    #[test]
    fn test_event_serializer_with_config() {
        let config = SerializationConfig::default();
        let fixture = EventSerializer::new().with_config(SerializationFormat::Json, config);
        assert_eq!(fixture.configs.len(), 1);
    }

    #[test]
    fn test_serialize_json() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();
        let actual = serializer.serialize(&fixture, Some(SerializationFormat::Json));
        assert!(actual.is_ok());
    }

    #[test]
    fn test_serialize_messagepack() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();
        let actual = serializer.serialize(&fixture, Some(SerializationFormat::MessagePack));
        assert!(actual.is_ok());
    }

    #[test]
    fn test_serialize_compact() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();
        let actual = serializer.serialize(&fixture, Some(SerializationFormat::JsonCompact));
        assert!(actual.is_ok());
    }

    #[test]
    fn test_deserialize_json() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();
        let serialized = serializer
            .serialize(&fixture, Some(SerializationFormat::Json))
            .unwrap();
        let actual = serializer.deserialize(&serialized, Some(SerializationFormat::Json));
        assert!(actual.is_ok());
        let deserialized = actual.unwrap();
        assert_eq!(deserialized.name(), fixture.name());
    }

    #[test]
    fn test_round_trip_serialization() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();

        for format in EventSerializer::supported_formats() {
            let serialized = serializer.serialize(&fixture, Some(format)).unwrap();
            let deserialized = serializer.deserialize(&serialized, Some(format)).unwrap();
            assert_eq!(deserialized.name(), fixture.name());
            assert_eq!(deserialized.data(), fixture.data());
        }
    }

    #[test]
    fn test_serialize_to_string() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();
        let actual = serializer.serialize_to_string(&fixture, Some(SerializationFormat::Json));
        assert!(actual.is_ok());
    }

    #[test]
    fn test_deserialize_from_string() {
        let serializer = EventSerializer::new();
        let fixture = create_test_event();
        let serialized = serializer
            .serialize_to_string(&fixture, Some(SerializationFormat::Json))
            .unwrap();
        let actual =
            serializer.deserialize_from_string(&serialized, Some(SerializationFormat::Json));
        assert!(actual.is_ok());
    }

    #[test]
    fn test_serialize_with_size_limit() {
        let config = SerializationConfig {
            max_size: Some(10), // Very small limit
            ..Default::default()
        };
        let serializer = EventSerializer::new().with_config(SerializationFormat::Json, config);
        let fixture = create_test_event();
        let actual = serializer.serialize(&fixture, Some(SerializationFormat::Json));
        assert!(actual.is_err());
    }

    #[test]
    fn test_serialized_event_creation() {
        let data = vec![1, 2, 3, 4];
        let fixture = SerializedEvent::new(data.clone(), SerializationFormat::Json);
        assert_eq!(fixture.data, data);
        assert_eq!(fixture.size, 4);
        assert_eq!(fixture.format, "json");
        assert_eq!(fixture.content_type, "application/json");
        assert!(!fixture.compressed);
    }

    #[test]
    fn test_serialized_event_with_compression() {
        let data = vec![1, 2, 3, 4];
        let fixture = SerializedEvent::new(data, SerializationFormat::Json).with_compression();
        assert!(fixture.compressed);
    }

    #[test]
    fn test_create_json_serializer() {
        let fixture = create_json_serializer();
        assert_eq!(fixture.default_format, SerializationFormat::Json);
    }

    #[test]
    fn test_create_compact_serializer() {
        let fixture = create_compact_serializer();
        assert_eq!(fixture.default_format, SerializationFormat::JsonCompact);
    }

    #[test]
    fn test_create_binary_serializer() {
        let fixture = create_binary_serializer();
        assert_eq!(fixture.default_format, SerializationFormat::MessagePack);
    }

    #[test]
    fn test_supported_formats() {
        let fixture = EventSerializer::supported_formats();
        assert_eq!(fixture.len(), 3);
        assert!(fixture.contains(&SerializationFormat::Json));
        assert!(fixture.contains(&SerializationFormat::JsonCompact));
        assert!(fixture.contains(&SerializationFormat::MessagePack));
    }
}
