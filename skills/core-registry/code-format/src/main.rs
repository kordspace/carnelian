use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        let error = json!({
            "success": false,
            "error": format!("Failed to read stdin: {}", e)
        });
        println!("{}", error);
        return;
    }

    let parsed: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            let error = json!({
                "success": false,
                "error": format!("Invalid JSON input: {}", e)
            });
            println!("{}", error);
            return;
        }
    };

    let code = match parsed.get("code").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: code"
            });
            println!("{}", error);
            return;
        }
    };

    let language = match parsed.get("lang").and_then(|v| v.as_str()) {
        Some(l) => l,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: lang"
            });
            println!("{}", error);
            return;
        }
    };

    let formatted = match language {
        "rust" => {
            match syn::parse_file(code) {
                Ok(syntax_tree) => prettyplease::unparse(&syntax_tree),
                Err(e) => {
                    let error = json!({
                        "success": false,
                        "error": format!("Rust parse error: {}", e)
                    });
                    println!("{}", error);
                    return;
                }
            }
        }
        "json" => {
            match serde_json::from_str::<Value>(code) {
                Ok(v) => match serde_json::to_string_pretty(&v) {
                    Ok(s) => s,
                    Err(e) => {
                        let error = json!({
                            "success": false,
                            "error": format!("JSON formatting error: {}", e)
                        });
                        println!("{}", error);
                        return;
                    }
                },
                Err(e) => {
                    let error = json!({
                        "success": false,
                        "error": format!("JSON parse error: {}", e)
                    });
                    println!("{}", error);
                    return;
                }
            }
        }
        "yaml" => {
            match serde_yaml::from_str::<Value>(code) {
                Ok(v) => match serde_yaml::to_string(&v) {
                    Ok(s) => s,
                    Err(e) => {
                        let error = json!({
                            "success": false,
                            "error": format!("YAML formatting error: {}", e)
                        });
                        println!("{}", error);
                        return;
                    }
                },
                Err(e) => {
                    let error = json!({
                        "success": false,
                        "error": format!("YAML parse error: {}", e)
                    });
                    println!("{}", error);
                    return;
                }
            }
        }
        _ => {
            normalize_indentation(code)
        }
    };

    let result = json!({
        "success": true,
        "data": {
            "formatted": formatted,
            "lang": language
        }
    });

    println!("{}", result);
}

fn normalize_indentation(code: &str) -> String {
    let mut result = String::new();
    let mut depth: i32 = 0;
    let mut prev_blank = false;

    for line in code.lines() {
        let trimmed = line.trim();
        
        if trimmed.is_empty() {
            if !prev_blank {
                result.push('\n');
                prev_blank = true;
            }
            continue;
        }
        
        prev_blank = false;

        if trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')') {
            depth = depth.saturating_sub(1);
        }

        result.push_str(&"    ".repeat(depth as usize));
        result.push_str(trimmed);
        result.push('\n');

        if trimmed.ends_with('{') || trimmed.ends_with('[') || trimmed.ends_with('(') {
            depth += 1;
        }
    }

    result
}
