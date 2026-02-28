//! Memory system performance benchmarks
//!
//! Benchmarks for:
//! - Vector similarity search
//! - Memory creation
//! - Memory updates
//! - Batch operations

use carnelian_core::memory::{MemoryManager, MemorySearchQuery, MemorySource};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sqlx::PgPool;
use tokio::runtime::Runtime;
use uuid::Uuid;

fn create_test_pool() -> PgPool {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/carnelian_test".to_string()
        });

        sqlx::PgPool::connect(&database_url)
            .await
            .expect("Failed to create test pool")
    })
}

fn test_embedding() -> Vec<f32> {
    vec![0.1; 1536]
}

fn benchmark_vector_search(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();
    let manager = MemoryManager::new(pool, None);

    // Setup: Create test memories
    rt.block_on(async {
        let identity_id = uuid::Uuid::new_v4();
        for i in 0..100 {
            let _ = manager
                .create_memory(
                    identity_id,
                    &format!("Test memory {i}"),
                    Some(format!("Summary {i}")),
                    MemorySource::Observation,
                    None,
                    0.5,
                    None,
                )
                .await;
        }
    });

    let embedding = test_embedding();

    let mut group = c.benchmark_group("vector_search");

    for limit in &[10, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.to_async(&rt).iter(|| async {
                manager
                    .search_memories(MemorySearchQuery {
                        embedding: black_box(embedding.clone()),
                        identity_id: Uuid::new_v4(),
                        min_similarity: 0.7,
                        limit,
                        sources: None,
                    })
                    .await
            });
        });
    }

    group.finish();
}

fn benchmark_memory_create(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();
    let manager = MemoryManager::new(pool, None);

    c.bench_function("memory_create", |b| {
        b.to_async(&rt).iter(|| async {
            manager
                .create_memory(
                    black_box(uuid::Uuid::new_v4()),
                    "Benchmark memory content",
                    Some("Benchmark summary".to_string()),
                    MemorySource::Observation,
                    None,
                    0.5,
                    Some(vec!["benchmark".to_string()]),
                )
                .await
        });
    });
}

fn benchmark_memory_list(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();
    let manager = MemoryManager::new(pool, None);

    // Setup: Create test memories
    rt.block_on(async {
        let identity_id = uuid::Uuid::new_v4();
        for i in 0..200 {
            let _ = manager
                .create_memory(
                    identity_id,
                    &format!("List test memory {i}"),
                    None,
                    MemorySource::Observation,
                    None,
                    0.5,
                    None,
                )
                .await;
        }
    });

    let mut group = c.benchmark_group("memory_list");

    for limit in &[10, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.to_async(&rt)
                .iter(|| async { manager.load_recent_memories(Uuid::new_v4(), limit).await });
        });
    }

    group.finish();
}

fn benchmark_concurrent_creates(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();

    c.bench_function("concurrent_creates_10", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];

            for i in 0..10 {
                let handle = tokio::spawn({
                    let pool_clone = pool.clone();
                    async move {
                        let mgr = MemoryManager::new(pool_clone, None);
                        mgr.create_memory(
                            Uuid::new_v4(),
                            &format!("Concurrent memory {i}"),
                            None,
                            MemorySource::Observation,
                            None,
                            0.5,
                            None,
                        )
                        .await
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_vector_search,
    benchmark_memory_create,
    benchmark_memory_list,
    benchmark_concurrent_creates
);
criterion_main!(benches);
