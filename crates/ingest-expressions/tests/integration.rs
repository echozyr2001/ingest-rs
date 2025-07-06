//! Integration tests for the expression engine

use ingest_expressions::prelude::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn test_basic_expression_engine_integration() {
    let fixture = ExpressionEngine::new();
    let context = EvaluationContext::new()
        .with_variable("x", json!(10))
        .with_variable("y", json!(20));

    let actual = fixture.evaluate_number("x + y", &context).await.unwrap();
    let expected = 30.0;

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_boolean_expression_integration() {
    let fixture = ExpressionEngine::new();
    let context = EvaluationContext::new()
        .with_variable("user_id", json!("123"))
        .with_variable("plan", json!("premium"));

    let actual = fixture
        .evaluate_bool("user_id == '123' && plan == 'premium'", &context)
        .await
        .unwrap();
    let expected = true;

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_string_expression_integration() {
    let fixture = ExpressionEngine::new();
    let context = EvaluationContext::new()
        .with_variable("first_name", json!("John"))
        .with_variable("last_name", json!("Doe"));

    let actual = fixture
        .evaluate_string("first_name + ' ' + last_name", &context)
        .await
        .unwrap();
    let expected = "John Doe".to_string();

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_expression_engine_with_event() {
    let event =
        ingest_core::Event::new("user.created", json!({"user_id": "123", "plan": "premium"}));
    let fixture = ExpressionEngine::new();
    let context = EvaluationContext::from_event(&event);

    let actual = fixture
        .evaluate_string("event_name", &context)
        .await
        .unwrap();
    let expected = "user.created".to_string();

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_expression_engine_with_security() {
    let security_config = SecurityConfig::new()
        .with_max_execution_time(std::time::Duration::from_millis(100))
        .with_max_memory_usage(1024 * 1024); // 1MB

    let fixture = ExpressionEngine::builder()
        .with_security(security_config)
        .build();

    let context = EvaluationContext::new().with_variable("value", json!(42));

    let actual = fixture
        .evaluate_number("value * 2", &context)
        .await
        .unwrap();
    let expected = 84.0;

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_expression_engine_with_functions() {
    let fixture = ExpressionEngine::new();
    let context = EvaluationContext::new().with_variable("text", json!("hello world"));

    // Test built-in function
    let actual = fixture
        .evaluate_number("size(text)", &context)
        .await
        .unwrap();
    let expected = 11.0;

    assert_eq!(actual, expected);
}
