//! Type system for expression values and operations

use crate::error::{ExpressionError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Expression value that can hold different types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExpressionValue {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<ExpressionValue>),
    /// Object with key-value pairs
    Object(HashMap<String, ExpressionValue>),
}

impl ExpressionValue {
    /// Create a null value
    pub fn null() -> Self {
        Self::Null
    }

    /// Create a boolean value
    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Create an integer value
    pub fn int(value: i64) -> Self {
        Self::Int(value)
    }

    /// Create a float value
    pub fn float(value: f64) -> Self {
        Self::Float(value)
    }

    /// Create a string value
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Create an array value
    pub fn array(values: Vec<ExpressionValue>) -> Self {
        Self::Array(values)
    }

    /// Create an object value
    pub fn object(values: HashMap<String, ExpressionValue>) -> Self {
        Self::Object(values)
    }

    /// Get the type of this value
    pub fn get_type(&self) -> ExpressionType {
        match self {
            Self::Null => ExpressionType::Null,
            Self::Bool(_) => ExpressionType::Bool,
            Self::Int(_) => ExpressionType::Int,
            Self::Float(_) => ExpressionType::Float,
            Self::String(_) => ExpressionType::String,
            Self::Array(_) => ExpressionType::Array,
            Self::Object(_) => ExpressionType::Object,
        }
    }

    /// Check if this value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Check if this value is truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Int(i) => *i != 0,
            Self::Float(f) => *f != 0.0,
            Self::String(s) => !s.is_empty(),
            Self::Array(a) => !a.is_empty(),
            Self::Object(o) => !o.is_empty(),
        }
    }

    /// Try to convert to boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to convert to integer
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Try to convert to float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to convert to number (float)
    pub fn as_number(&self) -> Option<f64> {
        self.as_float()
    }

    /// Try to convert to string
    pub fn as_string(&self) -> Option<String> {
        match self {
            Self::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Try to convert to array
    pub fn as_array(&self) -> Option<&Vec<ExpressionValue>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to convert to object
    pub fn as_object(&self) -> Option<&HashMap<String, ExpressionValue>> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Get property from object
    pub fn get_property(&self, key: &str) -> Result<ExpressionValue> {
        match self {
            Self::Object(obj) => obj
                .get(key)
                .cloned()
                .ok_or_else(|| ExpressionError::property_not_found(key, "Object")),
            _ => Err(ExpressionError::property_not_found(
                key,
                self.get_type().to_string(),
            )),
        }
    }

    /// Get array element by index
    pub fn get_index(&self, index: usize) -> Result<ExpressionValue> {
        match self {
            Self::Array(arr) => {
                if index < arr.len() {
                    Ok(arr[index].clone())
                } else {
                    Err(ExpressionError::index_out_of_bounds(index, arr.len()))
                }
            }
            _ => Err(ExpressionError::type_error(format!(
                "Cannot index into {}",
                self.get_type()
            ))),
        }
    }

    /// Convert to string representation
    pub fn to_string_repr(&self) -> String {
        match self {
            Self::Null => "null".to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::String(s) => s.clone(),
            Self::Array(a) => {
                let items: Vec<String> = a.iter().map(|v| v.to_string_repr()).collect();
                format!("[{}]", items.join(", "))
            }
            Self::Object(o) => {
                let items: Vec<String> = o
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_string_repr()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }

    /// Add two values
    pub fn add(&self, other: &ExpressionValue) -> Result<ExpressionValue> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Ok(Self::Int(a + b)),
            (Self::Float(a), Self::Float(b)) => Ok(Self::Float(a + b)),
            (Self::Int(a), Self::Float(b)) => Ok(Self::Float(*a as f64 + b)),
            (Self::Float(a), Self::Int(b)) => Ok(Self::Float(a + *b as f64)),
            (Self::String(a), Self::String(b)) => Ok(Self::String(format!("{a}{b}"))),
            _ => Err(ExpressionError::unsupported_operation(
                "+",
                self.get_type().to_string(),
                other.get_type().to_string(),
            )),
        }
    }

    /// Subtract two values
    pub fn subtract(&self, other: &ExpressionValue) -> Result<ExpressionValue> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Ok(Self::Int(a - b)),
            (Self::Float(a), Self::Float(b)) => Ok(Self::Float(a - b)),
            (Self::Int(a), Self::Float(b)) => Ok(Self::Float(*a as f64 - b)),
            (Self::Float(a), Self::Int(b)) => Ok(Self::Float(a - *b as f64)),
            _ => Err(ExpressionError::unsupported_operation(
                "-",
                self.get_type().to_string(),
                other.get_type().to_string(),
            )),
        }
    }

    /// Multiply two values
    pub fn multiply(&self, other: &ExpressionValue) -> Result<ExpressionValue> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Ok(Self::Int(a * b)),
            (Self::Float(a), Self::Float(b)) => Ok(Self::Float(a * b)),
            (Self::Int(a), Self::Float(b)) => Ok(Self::Float(*a as f64 * b)),
            (Self::Float(a), Self::Int(b)) => Ok(Self::Float(a * *b as f64)),
            _ => Err(ExpressionError::unsupported_operation(
                "*",
                self.get_type().to_string(),
                other.get_type().to_string(),
            )),
        }
    }

    /// Divide two values
    pub fn divide(&self, other: &ExpressionValue) -> Result<ExpressionValue> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => {
                if *b == 0 {
                    Err(ExpressionError::division_by_zero())
                } else {
                    Ok(Self::Float(*a as f64 / *b as f64))
                }
            }
            (Self::Float(a), Self::Float(b)) => {
                if *b == 0.0 {
                    Err(ExpressionError::division_by_zero())
                } else {
                    Ok(Self::Float(a / b))
                }
            }
            (Self::Int(a), Self::Float(b)) => {
                if *b == 0.0 {
                    Err(ExpressionError::division_by_zero())
                } else {
                    Ok(Self::Float(*a as f64 / b))
                }
            }
            (Self::Float(a), Self::Int(b)) => {
                if *b == 0 {
                    Err(ExpressionError::division_by_zero())
                } else {
                    Ok(Self::Float(a / *b as f64))
                }
            }
            _ => Err(ExpressionError::unsupported_operation(
                "/",
                self.get_type().to_string(),
                other.get_type().to_string(),
            )),
        }
    }

    /// Compare two values for equality
    pub fn equals(&self, other: &ExpressionValue) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Self::Int(a), Self::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
            (Self::Float(a), Self::Int(b)) => (a - *b as f64).abs() < f64::EPSILON,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            _ => false,
        }
    }

    /// Compare two values for less than
    pub fn less_than(&self, other: &ExpressionValue) -> Result<bool> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Ok(a < b),
            (Self::Float(a), Self::Float(b)) => Ok(a < b),
            (Self::Int(a), Self::Float(b)) => Ok((*a as f64) < *b),
            (Self::Float(a), Self::Int(b)) => Ok(*a < (*b as f64)),
            (Self::String(a), Self::String(b)) => Ok(a < b),
            _ => Err(ExpressionError::unsupported_operation(
                "<",
                self.get_type().to_string(),
                other.get_type().to_string(),
            )),
        }
    }

    /// Compare two values for greater than
    pub fn greater_than(&self, other: &ExpressionValue) -> Result<bool> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Ok(a > b),
            (Self::Float(a), Self::Float(b)) => Ok(a > b),
            (Self::Int(a), Self::Float(b)) => Ok((*a as f64) > *b),
            (Self::Float(a), Self::Int(b)) => Ok(*a > (*b as f64)),
            (Self::String(a), Self::String(b)) => Ok(a > b),
            _ => Err(ExpressionError::unsupported_operation(
                ">",
                self.get_type().to_string(),
                other.get_type().to_string(),
            )),
        }
    }
}

impl fmt::Display for ExpressionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_repr())
    }
}

impl From<serde_json::Value> for ExpressionValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Self::Float(f)
                } else {
                    Self::Null
                }
            }
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(arr) => {
                let values = arr.into_iter().map(ExpressionValue::from).collect();
                Self::Array(values)
            }
            serde_json::Value::Object(obj) => {
                let values = obj
                    .into_iter()
                    .map(|(k, v)| (k, ExpressionValue::from(v)))
                    .collect();
                Self::Object(values)
            }
        }
    }
}

impl From<ExpressionValue> for serde_json::Value {
    fn from(value: ExpressionValue) -> Self {
        match value {
            ExpressionValue::Null => serde_json::Value::Null,
            ExpressionValue::Bool(b) => serde_json::Value::Bool(b),
            ExpressionValue::Int(i) => serde_json::Value::Number(i.into()),
            ExpressionValue::Float(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            ExpressionValue::String(s) => serde_json::Value::String(s),
            ExpressionValue::Array(arr) => {
                let values = arr.into_iter().map(serde_json::Value::from).collect();
                serde_json::Value::Array(values)
            }
            ExpressionValue::Object(obj) => {
                let values = obj
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect();
                serde_json::Value::Object(values)
            }
        }
    }
}

/// Expression type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExpressionType {
    /// Null type
    Null,
    /// Boolean type
    Bool,
    /// Integer type
    Int,
    /// Float type
    Float,
    /// String type
    String,
    /// Array type
    Array,
    /// Object type
    Object,
}

impl fmt::Display for ExpressionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
        };
        write!(f, "{name}")
    }
}

/// Type registry for managing custom types
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    /// Custom type definitions
    custom_types: HashMap<String, ExpressionType>,
}

impl TypeRegistry {
    /// Create a new type registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a custom type
    pub fn register_type(&mut self, name: String, expr_type: ExpressionType) {
        self.custom_types.insert(name, expr_type);
    }

    /// Get a type by name
    pub fn get_type(&self, name: &str) -> Option<ExpressionType> {
        self.custom_types.get(name).copied()
    }

    /// Check if a type is registered
    pub fn has_type(&self, name: &str) -> bool {
        self.custom_types.contains_key(name)
    }

    /// Get all registered type names
    pub fn type_names(&self) -> Vec<&String> {
        self.custom_types.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_expression_value_creation() {
        let fixture = ExpressionValue::int(42);
        let actual = fixture.as_int().unwrap();
        let expected = 42;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_type() {
        let fixture = ExpressionValue::string("hello");
        let actual = fixture.get_type();
        let expected = ExpressionType::String;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_truthiness() {
        let fixture = ExpressionValue::string("");
        let actual = fixture.is_truthy();
        let expected = false;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_addition() {
        let fixture_a = ExpressionValue::int(10);
        let fixture_b = ExpressionValue::int(20);
        let actual = fixture_a.add(&fixture_b).unwrap();
        let expected = ExpressionValue::int(30);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_string_concatenation() {
        let fixture_a = ExpressionValue::string("Hello");
        let fixture_b = ExpressionValue::string(" World");
        let actual = fixture_a.add(&fixture_b).unwrap();
        let expected = ExpressionValue::string("Hello World");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_division_by_zero() {
        let fixture_a = ExpressionValue::int(10);
        let fixture_b = ExpressionValue::int(0);
        let actual = fixture_a.divide(&fixture_b);
        assert!(actual.is_err());
    }

    #[test]
    fn test_expression_value_equality() {
        let fixture_a = ExpressionValue::int(42);
        let fixture_b = ExpressionValue::float(42.0);
        let actual = fixture_a.equals(&fixture_b);
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_comparison() {
        let fixture_a = ExpressionValue::int(10);
        let fixture_b = ExpressionValue::int(20);
        let actual = fixture_a.less_than(&fixture_b).unwrap();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_property_access() {
        let mut obj = HashMap::new();
        obj.insert("name".to_string(), ExpressionValue::string("John"));
        let fixture = ExpressionValue::object(obj);
        let actual = fixture.get_property("name").unwrap();
        let expected = ExpressionValue::string("John");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_array_access() {
        let fixture = ExpressionValue::array(vec![
            ExpressionValue::int(1),
            ExpressionValue::int(2),
            ExpressionValue::int(3),
        ]);
        let actual = fixture.get_index(1).unwrap();
        let expected = ExpressionValue::int(2);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_from_json() {
        let fixture = json!({"name": "John", "age": 30});
        let actual = ExpressionValue::from(fixture);
        let mut expected_obj = HashMap::new();
        expected_obj.insert("name".to_string(), ExpressionValue::string("John"));
        expected_obj.insert("age".to_string(), ExpressionValue::int(30));
        let expected = ExpressionValue::object(expected_obj);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_to_json() {
        let mut obj = HashMap::new();
        obj.insert("name".to_string(), ExpressionValue::string("John"));
        obj.insert("age".to_string(), ExpressionValue::int(30));
        let fixture = ExpressionValue::object(obj);
        let actual = serde_json::Value::from(fixture);
        let expected = json!({"name": "John", "age": 30});
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_type_registry() {
        let mut fixture = TypeRegistry::new();
        fixture.register_type("User".to_string(), ExpressionType::Object);
        let actual = fixture.get_type("User").unwrap();
        let expected = ExpressionType::Object;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_type_display() {
        let fixture = ExpressionType::String;
        let actual = format!("{fixture}");
        let expected = "string";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_value_display() {
        let fixture = ExpressionValue::array(vec![
            ExpressionValue::int(1),
            ExpressionValue::string("hello"),
        ]);
        let actual = format!("{fixture}");
        let expected = "[1, hello]";
        assert_eq!(actual, expected);
    }
}
