use anyhow::Result;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ingest_core::{
    function::{Function, FunctionTrigger},
    Event,
};
use ingest_storage::{
    postgres::PostgresStorage,
    redis::{CacheStorage, RedisStorage},
};
use serde_json::json;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Storage operations benchmark fixtures
struct StorageFixture {
    postgres: PostgresStorage,
    redis: RedisStorage,
    test_event: Event,
    #[allow(dead_code)]
    test_function: Function,
}

impl StorageFixture {
    async fn new() -> Result<Self> {
        let postgres = PostgresStorage::new_in_memory().await?;
        let redis = RedisStorage::new_in_memory().await?;

        let test_event = Event::new("test.storage", json!({"data": "test_payload"}));

        let test_function =
            Function::new("test_function").add_trigger(FunctionTrigger::event("test.storage"));

        Ok(Self {
            postgres,
            redis,
            test_event,
            test_function,
        })
    }
}

/// Benchmark PostgreSQL event storage operations
fn bench_postgres_event_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("postgres_event_operations");

    // Single event insert
    group.bench_function("single_insert", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();
            fixture
                .postgres
                .store_event(&fixture.test_event)
                .await
                .unwrap();
        });
    });

    // Batch event insert
    for batch_size in [10, 100, 1000].iter() {
        let events: Vec<_> = (0..*batch_size)
            .map(|i| {
                Event::new(
                    format!("test.batch.{i}"),
                    json!({"data": format!("payload_{}", i)}),
                )
            })
            .collect();

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_insert", batch_size),
            &events,
            |b, events| {
                b.to_async(&rt).iter(|| async {
                    let fixture = StorageFixture::new().await.unwrap();
                    fixture
                        .postgres
                        .store_events_batch(events.clone())
                        .await
                        .unwrap();
                });
            },
        );
    }

    // Event retrieval (commented out since method doesn't exist)
    // group.bench_function("event_retrieval", |b| {
    //     b.to_async(&rt).iter(|| async {
    //         let fixture = StorageFixture::new().await.unwrap();
    //         // First store an event
    //         let event_id = fixture
    //             .postgres
    //             .store_event(&fixture.test_event)
    //             .await
    //             .unwrap();
    //         // Then retrieve it
    //         let _retrieved = fixture.postgres.get_event(&event_id).await.unwrap();
    //     });
    // });

    group.finish();
}

/// Benchmark PostgreSQL function storage operations
fn bench_postgres_function_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("postgres_function_operations");

    // Function storage (commented out since method doesn't exist)
    // group.bench_function("function_storage", |b| {
    //     b.to_async(&rt).iter(|| async {
    //         let fixture = StorageFixture::new().await.unwrap();
    //         fixture
    //             .postgres
    //             .store_function(&fixture.test_function)
    //             .await
    //             .unwrap();
    //     });
    // });

    // Function listing with pagination
    group.bench_function("function_listing", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();

            // List functions with pagination
            let _functions = fixture.postgres.list_functions(0, 10).await.unwrap();
        });
    });

    group.finish();
}

/// Benchmark Redis cache operations
fn bench_redis_cache_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("redis_cache_operations");

    // Cache set operations
    for value_size in [1024, 10240, 102400].iter() {
        let large_value = vec![b'x'; *value_size];

        group.throughput(Throughput::Bytes(*value_size as u64));
        group.bench_with_input(
            BenchmarkId::new("cache_set", value_size),
            &large_value,
            |b, value| {
                b.to_async(&rt).iter(|| async {
                    let fixture = StorageFixture::new().await.unwrap();
                    fixture.redis.set("test_key", value, None).await.unwrap();
                });
            },
        );
    }

    // Cache get operations
    group.bench_function("cache_get", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();

            // First set a value
            fixture
                .redis
                .set("test_key", "test_value".as_bytes(), None)
                .await
                .unwrap();

            // Then get it
            let _value: Option<Vec<u8>> = fixture.redis.get("test_key").await.unwrap();
        });
    });

    // Cache operations with TTL
    group.bench_function("cache_set_with_ttl", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();
            fixture
                .redis
                .set(
                    "test_key_ttl",
                    "test_value".as_bytes(),
                    Some(Duration::from_secs(60)),
                )
                .await
                .unwrap();
        });
    });

    group.finish();
}

/// Benchmark concurrent storage operations
fn bench_concurrent_storage_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_storage_operations");

    for concurrency in [1, 10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_postgres_inserts", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let fixture = StorageFixture::new().await.unwrap();

                    let tasks: Vec<_> = (0..concurrency)
                        .map(|i| {
                            let postgres = fixture.postgres.clone();
                            let event = Event::new(
                                format!("test.concurrent.{i}"),
                                json!({"data": format!("payload_{}", i)}),
                            );

                            tokio::spawn(async move {
                                postgres.store_event(&event).await.unwrap();
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

/// Benchmark storage query performance
fn bench_storage_query_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("storage_query_performance");

    // Complex queries with filtering
    group.bench_function("filtered_event_query", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();

            // Store events with different patterns
            for i in 0..100 {
                let event = Event::new(
                    if i % 2 == 0 { "test.even" } else { "test.odd" },
                    json!({"data": format!("payload_{}", i), "index": i}),
                );
                fixture.postgres.store_event(&event).await.unwrap();
            }

            // Query with filters
            let _events = fixture
                .postgres
                .query_events_by_name("test.even")
                .await
                .unwrap();
        });
    });

    // Aggregation queries
    group.bench_function("aggregation_query", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();

            // Store events for aggregation
            for i in 0..50 {
                let event = Event::new(
                    "test.aggregation",
                    json!({"value": i, "category": if i % 3 == 0 { "A" } else { "B" }}),
                );
                fixture.postgres.store_event(&event).await.unwrap();
            }

            // Perform aggregation
            let _stats = fixture.postgres.get_event_statistics().await.unwrap();
        });
    });

    group.finish();
}

/// Benchmark connection pool performance
fn bench_connection_pool_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("connection_pool_stress", |b| {
        b.to_async(&rt).iter(|| async {
            let fixture = StorageFixture::new().await.unwrap();

            // Simulate high connection usage
            let tasks: Vec<_> = (0..20)
                .map(|i| {
                    let postgres = fixture.postgres.clone();
                    let event = Event::new(
                        format!("test.pool.{i}"),
                        json!({"data": format!("payload_{}", i)}),
                    );

                    tokio::spawn(async move {
                        // Multiple operations per connection
                        for _ in 0..5 {
                            postgres.store_event(&event).await.unwrap();
                        }
                    })
                })
                .collect();

            for task in tasks {
                task.await.unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_postgres_event_operations,
    bench_postgres_function_operations,
    bench_redis_cache_operations,
    bench_concurrent_storage_operations,
    bench_storage_query_performance,
    bench_connection_pool_performance
);

criterion_main!(benches);

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_storage_fixture() {
        use super::*;
        use pretty_assertions::assert_eq;

        let fixture = StorageFixture::new().await.unwrap();

        assert_eq!(fixture.test_event.name, "test.storage");
        assert_eq!(fixture.test_function.name, "test_function");
    }

    #[tokio::test]
    async fn test_postgres_operations() {
        let fixture = StorageFixture::new().await.unwrap();

        let event_id = fixture
            .postgres
            .store_event(&fixture.test_event)
            .await
            .unwrap();

        // Note: get_event method doesn't exist, so we just verify store_event works
        assert!(!event_id.is_empty());
    }

    #[tokio::test]
    async fn test_redis_operations() {
        let fixture = StorageFixture::new().await.unwrap();

        fixture
            .redis
            .set("test", "value".as_bytes(), None)
            .await
            .unwrap();
        let value: Option<Vec<u8>> = fixture.redis.get("test").await.unwrap();

        assert_eq!(value, Some("value".as_bytes().to_vec()));
    }
}
