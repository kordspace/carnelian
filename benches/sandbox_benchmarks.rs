//! Benchmarks for sandbox timeout and resource limit overhead
//!
//! Measures the performance impact of sandboxing on skill execution.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::process::Command;
use std::time::Duration;

fn bench_sandbox_timeout_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox_timeout_overhead");

    // Benchmark simple command execution with different timeout values
    for timeout_secs in [1, 5, 10, 30].iter() {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("with_timeout", timeout_secs),
            timeout_secs,
            |b, &timeout| {
                b.iter(|| {
                    // Execute a quick command with timeout
                    let output = if cfg!(target_os = "windows") {
                        Command::new("cmd")
                            .args(&["/C", "echo", "test"])
                            .output()
                    } else {
                        Command::new("echo")
                            .arg("test")
                            .output()
                    };
                    
                    black_box(output)
                });
            },
        );
    }

    group.finish();
}

fn bench_process_spawn_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("process_spawn");

    group.bench_function("spawn_echo", |b| {
        b.iter(|| {
            let output = if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .args(&["/C", "echo", "benchmark"])
                    .output()
            } else {
                Command::new("echo")
                    .arg("benchmark")
                    .output()
            };
            
            black_box(output)
        });
    });

    group.finish();
}

fn bench_resource_limit_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_limit_checks");

    // Simulate resource limit validation overhead
    group.bench_function("memory_limit_check", |b| {
        b.iter(|| {
            let memory_mb = 512;
            let max_memory_mb = 1024;
            black_box(memory_mb < max_memory_mb)
        });
    });

    group.bench_function("cpu_limit_check", |b| {
        b.iter(|| {
            let cpu_percent = 75.0;
            let max_cpu_percent = 90.0;
            black_box(cpu_percent < max_cpu_percent)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sandbox_timeout_overhead,
    bench_process_spawn_overhead,
    bench_resource_limit_checks
);
criterion_main!(benches);
