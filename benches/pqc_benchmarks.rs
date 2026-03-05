//! Benchmarks for PQC cryptographic operations
//!
//! Measures performance of batch verification, Merkle tree operations,
//! and hybrid signature schemes.

use carnelian_magic::{
    batch_verify_hybrid, sequential_verify_hybrid, EntropyProvider, HybridSigningKey,
    MemoryMerkleTree, MixedEntropyProvider,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;

fn bench_batch_vs_sequential_verify(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let provider: Arc<dyn EntropyProvider> = Arc::new(MixedEntropyProvider::new_os_only());
    let key = rt.block_on(async {
        HybridSigningKey::generate_with_entropy(&provider)
            .await
            .unwrap()
    });

    let mut group = c.benchmark_group("hybrid_signature_verification");

    for size in [10, 50, 100, 200].iter() {
        let messages: Vec<Vec<u8>> = (0..*size)
            .map(|i| format!("Benchmark message {}", i).into_bytes())
            .collect();

        let signatures: Vec<_> = messages.iter().map(|msg| key.sign(msg)).collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Benchmark batch verification (parallel with Rayon)
        group.bench_with_input(
            BenchmarkId::new("batch_verify", size),
            size,
            |b, _| {
                b.iter(|| {
                    batch_verify_hybrid(
                        black_box(&key),
                        black_box(&messages),
                        black_box(&signatures),
                    )
                });
            },
        );

        // Benchmark sequential verification
        group.bench_with_input(
            BenchmarkId::new("sequential_verify", size),
            size,
            |b, _| {
                b.iter(|| {
                    sequential_verify_hybrid(
                        black_box(&key),
                        black_box(&messages),
                        black_box(&signatures),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_merkle_proof_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_proof_generation");

    for size in [10, 100, 1000, 10000].iter() {
        let leaves: Vec<[u8; 32]> = (0..*size)
            .map(|i| {
                let data = format!("Memory entry {}", i);
                *blake3::hash(data.as_bytes()).as_bytes()
            })
            .collect();

        let tree = MemoryMerkleTree::new(leaves.clone());

        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("generate_proof", size),
            size,
            |b, _| {
                b.iter(|| {
                    // Generate proof for middle element
                    let index = size / 2;
                    tree.generate_proof(black_box(index))
                });
            },
        );
    }

    group.finish();
}

fn bench_merkle_proof_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_proof_verification");

    for size in [10, 100, 1000, 10000].iter() {
        let leaves: Vec<[u8; 32]> = (0..*size)
            .map(|i| {
                let data = format!("Memory entry {}", i);
                *blake3::hash(data.as_bytes()).as_bytes()
            })
            .collect();

        let tree = MemoryMerkleTree::new(leaves.clone());
        let index = size / 2;
        let proof = tree.generate_proof(index).unwrap();
        let leaf_hash = &leaves[index];

        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("verify_proof", size),
            size,
            |b, _| {
                b.iter(|| {
                    tree.verify_proof(black_box(leaf_hash), black_box(&proof))
                });
            },
        );
    }

    group.finish();
}

fn bench_merkle_tree_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_tree_construction");

    for size in [10, 100, 1000, 10000].iter() {
        let leaves: Vec<[u8; 32]> = (0..*size)
            .map(|i| {
                let data = format!("Memory entry {}", i);
                *blake3::hash(data.as_bytes()).as_bytes()
            })
            .collect();

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("construct_tree", size),
            size,
            |b, _| {
                b.iter(|| {
                    MemoryMerkleTree::new(black_box(leaves.clone()))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_vs_sequential_verify,
    bench_merkle_proof_generation,
    bench_merkle_proof_verification,
    bench_merkle_tree_construction
);
criterion_main!(benches);
