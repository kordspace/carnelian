//! Skill execution performance benchmarks
//!
//! Benchmarks for:
//! - WASM skill loading
//! - Skill execution
//! - Concurrent skill execution

use carnelian_core::skills::skill_trait::SkillInput;
use carnelian_core::skills::wasm_runtime::WasmSkillRuntime;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json::json;

fn test_skill_input() -> SkillInput {
    SkillInput {
        action: "execute".to_string(),
        params: json!({
            "data": [1, 2, 3, 4, 5],
            "operation": "sum"
        }),
        identity_id: None,
        correlation_id: None,
    }
}

fn benchmark_wasm_runtime_creation(c: &mut Criterion) {
    c.bench_function("wasm_runtime_create", |b| {
        b.iter(WasmSkillRuntime::new);
    });
}

fn benchmark_skill_input_serialization(c: &mut Criterion) {
    let input = test_skill_input();

    c.bench_function("skill_input_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&input)));
    });
}

fn benchmark_skill_input_deserialization(c: &mut Criterion) {
    let input = test_skill_input();
    let serialized = serde_json::to_string(&input).unwrap();

    c.bench_function("skill_input_deserialize", |b| {
        b.iter(|| serde_json::from_str::<SkillInput>(black_box(&serialized)));
    });
}

criterion_group!(
    benches,
    benchmark_wasm_runtime_creation,
    benchmark_skill_input_serialization,
    benchmark_skill_input_deserialization
);
criterion_main!(benches);
