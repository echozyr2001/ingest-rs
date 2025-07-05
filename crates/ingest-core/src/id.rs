use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(String);

impl Id {
    /// Create a new ID from a string
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the string representation of the ID
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to owned string
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Id {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<uuid::Uuid> for Id {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid.to_string())
    }
}

/// Generate a new unique ID
pub fn generate_id() -> Id {
    Id(uuid::Uuid::new_v4().to_string())
}

/// Generate a new unique ID with a prefix
pub fn generate_id_with_prefix(prefix: &str) -> Id {
    Id(format!("{}_{}", prefix, uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_id_creation() {
        let fixture = "test-id-123";
        let actual = Id::new(fixture);
        let expected = Id("test-id-123".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_id_display() {
        let fixture = Id::new("test-id");
        let actual = format!("{}", fixture);
        let expected = "test-id";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_id_from_string() {
        let fixture = "test-id".to_string();
        let actual = Id::from(fixture);
        let expected = Id::new("test-id");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_id_from_str() {
        let fixture = "test-id";
        let actual = Id::from(fixture);
        let expected = Id::new("test-id");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_id_from_uuid() {
        let fixture = uuid::Uuid::new_v4();
        let actual = Id::from(fixture);
        let expected = Id::new(fixture.to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_generate_id() {
        let actual = generate_id();
        assert!(!actual.as_str().is_empty());
        assert!(uuid::Uuid::parse_str(actual.as_str()).is_ok());
    }

    #[test]
    fn test_generate_id_with_prefix() {
        let fixture = "event";
        let actual = generate_id_with_prefix(fixture);
        assert!(actual.as_str().starts_with("event_"));
    }

    #[test]
    fn test_id_serialization() {
        let fixture = Id::new("test-id");
        let actual = serde_json::to_string(&fixture).unwrap();
        let expected = "\"test-id\"";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_id_deserialization() {
        let fixture = "\"test-id\"";
        let actual: Id = serde_json::from_str(fixture).unwrap();
        let expected = Id::new("test-id");
        assert_eq!(actual, expected);
    }
}
