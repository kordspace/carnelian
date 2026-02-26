use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    data: JsonValue,
    #[serde(default)]
    pretty: bool,
}

#[derive(Serialize)]
struct Output {
    toml_data: String,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn json_to_toml(json: &JsonValue) -> Result<toml::Value, String> {
    match json {
        JsonValue::Null => Ok(toml::Value::String(String::new())),
        JsonValue::Bool(b) => Ok(toml::Value::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Err("Invalid number format".to_string())
            }
        }
        JsonValue::String(s) => Ok(toml::Value::String(s.clone())),
        JsonValue::Array(arr) => {
            let toml_arr: Result<Vec<toml::Value>, String> =
                arr.iter().map(json_to_toml).collect();
            Ok(toml::Value::Array(toml_arr?))
        }
        JsonValue::Object(map) => {
            let mut toml_map = toml::map::Map::new();
            for (key, value) in map {
                toml_map.insert(key.clone(), json_to_toml(value)?);
            }
            Ok(toml::Value::Table(toml_map))
        }
    }
}

fn main() {
    let mut input_str = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input_str) {
        let error = ErrorOutput {
            error: format!("Failed to read input: {}", e),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let input: Input = match serde_json::from_str(&input_str) {
        Ok(i) => i,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid JSON input: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let toml_value = match json_to_toml(&input.data) {
        Ok(v) => v,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to convert JSON to TOML: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let toml_data = if input.pretty {
        toml::to_string_pretty(&toml_value)
    } else {
        toml::to_string(&toml_value)
    };

    match toml_data {
        Ok(data) => {
            let output = Output { toml_data: data };
            println!("{}", serde_json::to_string(&output).unwrap());
        }
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to serialize TOML: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }
}
