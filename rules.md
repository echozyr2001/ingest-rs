## Writing Domain Types:
- Use `derive_setters` to derive setters and use the `strip_option` and the `into` attributes on the struct types.

## Refactoring:
- If asked to fix failing tests, always confirm whether to update the implementation or the tests.

## Workspace Dependencies:
- For some public dependencies, you should use `Workspace Dependencies` first. Only add a dependency to the Cargo.toml of a crate when it is only used by one crate.
- When choosing dependencies, you should choose the latest version from https://crates.io/

## Code Style and Standards

### General Rust Guidelines
- Use `#[derive(Debug, Clone, PartialEq, Eq)]` on data structures where appropriate
- Prefer `anyhow::Result<T>` for application errors, `thiserror` for library errors
- Use `#[async_trait]` for async traits
- Always use `Arc<dyn Trait>` for shared trait objects
- Implement `Send + Sync` for all service types

### Error Handling
```rust
// ✅ Good: Use anyhow for application code
use anyhow::{Context, Result};

pub async fn process_event(event: Event) -> Result<ProcessedEvent> {
    validate_event(&event).context("Event validation failed")?;
    // ...
}

// ✅ Good: Use thiserror for library errors
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("Redis connection failed: {0}")]
    RedisConnection(#[from] redis::RedisError),
    
    #[error("State not found for run ID: {run_id}")]
    StateNotFound { run_id: String },
}
```

### Testing Standards
```rust
// ✅ Always use this exact testing pattern
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_event_processing() {
    // Arrange: Set up test fixtures
    let fixture = EventFixture::new()
        .with_event_name("user.created")
        .with_data(json!({"user_id": "123"}));
    
    // Act: Execute the operation
    let actual = process_event(fixture.event()).await.unwrap();
    
    // Assert: Verify expected results
    let expected = ProcessedEvent {
        id: fixture.event().id.clone(),
        status: ProcessingStatus::Success,
        // ...
    };
    assert_eq!(actual, expected);
}
```

### Performance Optimization Patterns
```rust
// ✅ Use efficient data structures
use dashmap::DashMap;           // For concurrent hash maps
use parking_lot::RwLock;        // For high-performance locks
use bytes::Bytes;               // For zero-copy string operations
use crossbeam_channel::unbounded; // For high-performance channels

// ✅ Object pooling for hot paths
use object_pool::Pool;

// ✅ Async-friendly logging
use tracing::{info, warn, error, instrument};

#[instrument(skip(self), fields(run_id = %identifier.run_id))]
pub async fn execute_step(&self, identifier: &Identifier) -> Result<StepResult> {
    // ...
}
```

## Common Patterns and Anti-Patterns

### Do This
- Use `Arc<dyn Trait>` for shared service dependencies
- Always implement `Clone` for managers and services
- Use structured logging with context
- Prefer `bytes::Bytes` over `String` for binary data
- Use connection pooling for Redis and PostgreSQL
- Implement proper error propagation with context

### Don't Do This  
- Don't use `unwrap()` in production code (except tests)
- Don't implement `From` for domain error conversions
- Don't block async runtime with synchronous operations
- Don't use `std::sync::Mutex` in async code (use `tokio::sync::Mutex`)
- Don't ignore clippy warnings
- Don't break compatibility requirements for performance

## Development Workflow

### Code Quality
```bash
# ✅ Always run these before committing
cargo +nightly fmt --all
cargo +nightly clippy --fix --allow-staged --allow-dirty --workspace
cargo test --all-features  
cargo audit
```