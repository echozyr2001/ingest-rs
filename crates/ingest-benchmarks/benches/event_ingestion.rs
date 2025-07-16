use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ingest_core::Event;
use ingest_events::{EventBatch, EventIngestionService};
use serde_json::json;
use tokio::runtime::Runtime;

/// Event ingestion benchmark fixtures
struct EventFixture {
    single_event: Event,
    event_batch_small: EventBatch,
    event_batch_large: EventBatch,
}

impl EventFixture {
    fn new() -> Self {
        let single_event = Event::new("test.event", json!({"data": "test_payload"}));

        let event_batch_small = EventBatch::new(
            (0..10)
                .map(|i| {
                    Event::new(
                        format!("test.event.{i}"),
                        json!({"data": format!("payload_{}", i)}),
                    )
                })
                .collect(),
        );

        let event_batch_large = EventBatch::new(
            (0..1000)
                .map(|i| {
                    Event::new(
                        format!("test.event.{i}"),
                        json!({"data": format!("payload_{}", i)}),
                    )
                })
                .collect(),
        );

        Self {
            single_event,
            event_batch_small,
            event_batch_large,
        }
    }
}

/// Benchmark single event ingestion
fn bench_single_event_ingestion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = EventFixture::new();

    c.bench_function("single_event_ingestion", |b| {
        b.to_async(&rt).iter(|| async {
            let service = EventIngestionService::new_in_memory().await.unwrap();
            service
                .ingest_event(fixture.single_event.clone())
                .await
                .unwrap();
        });
    });
}

/// Benchmark batch event ingestion
fn bench_batch_event_ingestion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = EventFixture::new();

    let mut group = c.benchmark_group("batch_event_ingestion");

    for batch_size in [10, 100, 1000].iter() {
        let batch = match *batch_size {
            10 => fixture.event_batch_small.clone(),
            _ => EventBatch::new(
                (0..*batch_size)
                    .map(|i| {
                        Event::new(
                            format!("test.event.{i}"),
                            json!({"data": format!("payload_{}", i)}),
                        )
                    })
                    .collect(),
            ),
        };

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            &batch,
            |b, batch| {
                b.to_async(&rt).iter(|| async {
                    let service = EventIngestionService::new_in_memory().await.unwrap();
                    service.ingest_batch(batch.clone()).await.unwrap();
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent event ingestion
fn bench_concurrent_event_ingestion(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = EventFixture::new();

    let mut group = c.benchmark_group("concurrent_event_ingestion");

    for concurrency in [1, 10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrency", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let service = EventIngestionService::new_in_memory().await.unwrap();

                    let tasks: Vec<_> = (0..concurrency)
                        .map(|_| {
                            let service = service.clone();
                            let event = fixture.single_event.clone();
                            tokio::spawn(async move {
                                service.ingest_event(event).await.unwrap();
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

/// Benchmark event ingestion throughput over time
fn bench_sustained_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("sustained_throughput_10s", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let fixture = EventFixture::new();
            let service = EventIngestionService::new_in_memory().await.unwrap();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                service
                    .ingest_event(fixture.single_event.clone())
                    .await
                    .unwrap();
            }

            start.elapsed()
        });
    });
}

/// Benchmark memory usage during ingestion
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let fixture = EventFixture::new();

    c.bench_function("memory_usage_large_batch", |b| {
        b.to_async(&rt).iter(|| async {
            let service = EventIngestionService::new_in_memory().await.unwrap();

            // Ingest large batch and measure memory impact
            for _ in 0..10 {
                service
                    .ingest_batch(fixture.event_batch_large.clone())
                    .await
                    .unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_single_event_ingestion,
    bench_batch_event_ingestion,
    bench_concurrent_event_ingestion,
    bench_sustained_throughput,
    bench_memory_usage
);

criterion_main!(benches);

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_event_fixture() {
        use super::*;

        let fixture = EventFixture::new();

        assert_eq!(fixture.single_event.name(), "test.event");
        assert_eq!(fixture.event_batch_small.events().len(), 10);
        assert_eq!(fixture.event_batch_large.events().len(), 1000);
    }

    #[tokio::test]
    async fn test_benchmark_setup() {
        let service = EventIngestionService::new_in_memory().await.unwrap();
        let fixture = EventFixture::new();

        let result = service.ingest_event(fixture.single_event.clone()).await;
        assert!(result.is_ok());
    }
}
