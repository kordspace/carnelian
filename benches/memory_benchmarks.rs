//! Memory system performance benchmarks
//!
//! Benchmarks for:
//! - Vector similarity search
//! - Memory creation
//! - Memory updates
//! - Batch operations

use carnelian_common::types::CreateMemoryRequest;
use carnelian_core::memory::MemoryManager;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use serde_json::json;
use sqlx::PgPool;
use tokio::runtime::Runtime;

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
    let manager = MemoryManager::new(pool.clone());

    // Setup: Create test memories
    rt.block_on(async {
        for i in 0..100 {
            let _ = manager
                .create(CreateMemoryRequest {
                    content: format!("Test memory {}", i),
                    metadata: json!({"index": i}),
                    tags: vec![],
                    identity_id: None,
                })
                .await;
        }
    });

    let embedding = test_embedding();

    let mut group = c.benchmark_group("vector_search");

    for limit in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.to_async(&rt).iter(|| async {
                manager
                    .search_similar(black_box(&embedding), limit, None)
                    .await
            });
        });
    }

    group.finish();
}

fn benchmark_memory_create(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();
    let manager = MemoryManager::new(pool);

    c.bench_function("memory_create", |b| {
        b.to_async(&rt).iter(|| async {
            manager
                .create(black_box(CreateMemoryRequest {
                    content: "Benchmark memory content".to_string(),
                    metadata: json!({"benchmark": true}),
                    tags: vec!["benchmark".to_string()],
                    identity_id: None,
                }))
                .await
        });
    });
}

fn benchmark_memory_list(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();
    let manager = MemoryManager::new(pool.clone());

    // Setup: Create test memories
    rt.block_on(async {
        for i in 0..200 {
            let _ = manager
                .create(CreateMemoryRequest {
                    content: format!("List test memory {}", i),
                    metadata: json!({}),
                    tags: vec![],
                    identity_id: None,
                })
                .await;
        }
    });

    let mut group = c.benchmark_group("memory_list");

    for limit in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.to_async(&rt)
                .iter(|| async { manager.list(None, None, limit, 0).await });
        });
    }

    group.finish();
}

fn benchmark_concurrent_creates(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = create_test_pool();
    let manager = MemoryManager::new(pool);

    c.bench_function("concurrent_creates_10", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handles = vec![];

            for i in 0..10 {
                let manager_clone = manager.clone();
                let handle = tokio::spawn(async move {
                    manager_clone
                        .create(CreateMemoryRequest {
                            content: format!("Concurrent memory {}", i),
                            metadata: json!({}),
                            tags: vec![],
                            identity_id: None,
                        })
                        .await
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
