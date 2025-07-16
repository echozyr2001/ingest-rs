use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ingest_core::{
    function::{Function, FunctionTrigger},
    Event,
};
use ingest_execution::{
    engine::{DefaultExecutionEngine, ExecutionEngine},
    types::ExecutionRequest,
};
use serde_json::json;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Function execution benchmark fixtures
struct FunctionFixture {
    simple_function: Function,
    complex_function: Function,
    multi_step_function: Function,
    test_event: Event,
}

impl FunctionFixture {
    fn new() -> Self {
        let simple_function =
            Function::new("simple_function").add_trigger(FunctionTrigger::event("test.simple"));

        let complex_function =
            Function::new("complex_function").add_trigger(FunctionTrigger::event("test.complex"));

        let multi_step_function =
            Function::new("multi_step_function").add_trigger(FunctionTrigger::event("test.multi"));

        let test_event = Event::new("test.execution", json!({"data": "test_payload"}));

        Self {
            simple_function,
            complex_function,
            multi_step_function,
            test_event,
        }
    }
}

/// Benchmark simple function execution
fn bench_simple_function_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = FunctionFixture::new();

    c.bench_function("simple_function_execution", |b| {
        b.to_async(&rt).iter(|| async {
            let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();

            let request =
                ExecutionRequest::new(fixture.simple_function.clone(), fixture.test_event.clone());

            engine.execute_function(request).await.unwrap();
        });
    });
}

/// Benchmark complex function execution
fn bench_complex_function_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = FunctionFixture::new();

    c.bench_function("complex_function_execution", |b| {
        b.to_async(&rt).iter(|| async {
            let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();

            let request =
                ExecutionRequest::new(fixture.complex_function.clone(), fixture.test_event.clone());

            engine.execute_function(request).await.unwrap();
        });
    });
}

/// Benchmark concurrent function execution
fn bench_concurrent_function_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = FunctionFixture::new();

    let mut group = c.benchmark_group("concurrent_function_execution");

    for concurrency in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrency", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let engine = Arc::new(DefaultExecutionEngine::new_in_memory().await.unwrap());

                    let tasks: Vec<_> = (0..concurrency)
                        .map(|_| {
                            let engine = engine.clone();
                            let request = ExecutionRequest::new(
                                fixture.simple_function.clone(),
                                fixture.test_event.clone(),
                            );
                            tokio::spawn(async move {
                                engine.execute_function(request).await.unwrap();
                            })
                        })
                        .collect();

                    for task in tasks {
                        task.await.unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark step execution performance
fn bench_step_execution(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = FunctionFixture::new();

    let mut group = c.benchmark_group("step_execution");

    for step_count in [1, 5, 10, 20].iter() {
        let function = Function::new(format!("test_function_{step_count}"))
            .add_trigger(FunctionTrigger::event("test.steps"));

        group.throughput(Throughput::Elements(*step_count as u64));
        group.bench_with_input(
            BenchmarkId::new("step_count", step_count),
            &function,
            |b, function| {
                b.to_async(&rt).iter(|| async {
                    let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();
                    let request =
                        ExecutionRequest::new(function.clone(), fixture.test_event.clone());

                    engine.execute_function(request).await.unwrap();
                });
            },
        );
    }
    group.finish();
}

/// Benchmark function execution with different payload sizes
fn bench_payload_size_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = FunctionFixture::new();

    let mut group = c.benchmark_group("payload_size_impact");

    for payload_size in [1024, 10240, 102400, 1024000].iter() {
        let large_payload = "x".repeat(*payload_size);
        let event = Event::new("test.large_payload", json!({"data": large_payload}));

        group.throughput(Throughput::Bytes(*payload_size as u64));
        group.bench_with_input(
            BenchmarkId::new("payload_bytes", payload_size),
            &event,
            |b, event| {
                b.to_async(&rt).iter(|| async {
                    let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();
                    let request =
                        ExecutionRequest::new(fixture.simple_function.clone(), event.clone());

                    engine.execute_function(request).await.unwrap();
                });
            },
        );
    }
    group.finish();
}

/// Benchmark sustained execution throughput
fn bench_sustained_execution_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("sustained_execution_throughput", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let fixture = FunctionFixture::new();
            let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let request = ExecutionRequest::new(
                    fixture.simple_function.clone(),
                    fixture.test_event.clone(),
                );
                engine.execute_function(request).await.unwrap();
            }

            start.elapsed()
        });
    });
}

/// Benchmark memory usage during execution
fn bench_execution_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = FunctionFixture::new();

    c.bench_function("execution_memory_usage", |b| {
        b.to_async(&rt).iter(|| async {
            let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();

            // Execute multiple functions to test memory usage
            for _ in 0..100 {
                let request = ExecutionRequest::new(
                    fixture.multi_step_function.clone(),
                    fixture.test_event.clone(),
                );
                engine.execute_function(request).await.unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_simple_function_execution,
    bench_complex_function_execution,
    bench_concurrent_function_execution,
    bench_step_execution,
    bench_payload_size_impact,
    bench_sustained_execution_throughput,
    bench_execution_memory_usage
);

criterion_main!(benches);

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_function_fixture() {
        use super::*;
        use pretty_assertions::assert_eq;

        let fixture = FunctionFixture::new();

        assert_eq!(fixture.simple_function.name, "simple_function");
        assert_eq!(fixture.simple_function.triggers.len(), 1);
        assert_eq!(fixture.complex_function.triggers.len(), 1);
        assert_eq!(fixture.multi_step_function.triggers.len(), 1);
    }

    #[tokio::test]
    async fn test_execution_setup() {
        let engine = DefaultExecutionEngine::new_in_memory().await.unwrap();
        let fixture = FunctionFixture::new();

        let request =
            ExecutionRequest::new(fixture.simple_function.clone(), fixture.test_event.clone());

        let result = engine.execute_function(request).await;
        assert!(result.is_ok());
    }
}
