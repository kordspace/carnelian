use carnelian_core::skills::wasm_runtime::WasmSkillRuntime;
use carnelian_core::skills::SkillInput;
use serde_json::json;
use std::path::PathBuf;

// Shared helpers

fn wasm_path(skill_name: &str) -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("../../skills/core-registry")
        .join(skill_name)
        .join(format!("{skill_name}.wasm"))
}

fn make_runtime() -> WasmSkillRuntime {
    WasmSkillRuntime::new().unwrap()
}

fn skill_input(params: serde_json::Value) -> SkillInput {
    SkillInput {
        action: "invoke".into(),
        params,
        identity_id: None,
        correlation_id: None,
    }
}

// WASM Skill Tests

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_hello_wasm() {
    let runtime = make_runtime();
    let path = wasm_path("hello-wasm");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "hello-wasm").unwrap();
    let output = runtime
        .invoke("hello-wasm", skill_input(json!({})), vec![])
        .await
        .unwrap();

    assert!(output.success);
    assert_eq!(output.data["data"]["message"], "Hello from WASM!");
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_markdown_parse() {
    let runtime = make_runtime();
    let path = wasm_path("markdown-parse");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "markdown-parse").unwrap();
    let output = runtime
        .invoke(
            "markdown-parse",
            skill_input(json!({"content": "# Title\n\nParagraph text."})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert!(output.data["data"]["ast"].is_array());
    assert_eq!(output.data["data"]["headings"][0]["text"], "Title");
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_json_transform_field() {
    let runtime = make_runtime();
    let path = wasm_path("json-transform");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "json-transform").unwrap();
    let output = runtime
        .invoke(
            "json-transform",
            skill_input(json!({"data": {"name": "carnelian"}, "query": ".name"})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert_eq!(output.data["data"]["result"][0], "carnelian");
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_json_transform_identity() {
    let runtime = make_runtime();
    let path = wasm_path("json-transform");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "json-transform").unwrap();
    let output = runtime
        .invoke(
            "json-transform",
            skill_input(json!({"data": [1,2,3], "query": "."})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert!(output.data["data"]["result"][0].is_array());
    assert_eq!(
        output.data["data"]["result"][0].as_array().unwrap().len(),
        3
    );
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_yaml_parse() {
    let runtime = make_runtime();
    let path = wasm_path("yaml-parse");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "yaml-parse").unwrap();
    let output = runtime
        .invoke(
            "yaml-parse",
            skill_input(json!({"content": "name: carnelian\nversion: 1"})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert_eq!(output.data["data"]["json"]["name"], "carnelian");
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_text_search() {
    let runtime = make_runtime();
    let path = wasm_path("text-search");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "text-search").unwrap();
    let output = runtime
        .invoke(
            "text-search",
            skill_input(json!({"text": "foo bar baz", "pattern": "\\b\\w{3}\\b"})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert_eq!(output.data["data"]["count"].as_u64().unwrap(), 3);
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_hash_file() {
    let runtime = make_runtime();
    let path = wasm_path("hash-file");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "hash-file").unwrap();
    let output = runtime
        .invoke(
            "hash-file",
            skill_input(json!({"content": "hello world"})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert!(output.data["data"]["hash"].is_string());
    assert_eq!(output.data["data"]["algorithm"], "blake3");
}

#[tokio::test]
#[ignore = "requires compiled .wasm binaries — run with: cargo test --test wasm_skill_tests -- --ignored"]
async fn test_code_format_json() {
    let runtime = make_runtime();
    let path = wasm_path("code-format");

    if !path.exists() {
        println!("Skipping test: {} does not exist", path.display());
        return;
    }

    runtime.load(&path, "code-format").unwrap();
    let output = runtime
        .invoke(
            "code-format",
            skill_input(json!({"code": "{\"a\":1,\"b\":2}", "lang": "json"})),
            vec![],
        )
        .await
        .unwrap();

    assert!(output.success);
    assert!(output.data["data"]["formatted"]
        .as_str()
        .unwrap()
        .contains("\"a\":"));
    assert_eq!(output.data["data"]["lang"], "json");
}
