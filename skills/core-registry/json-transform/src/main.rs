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

    let input_data = match parsed.get("data") {
        Some(v) => v.clone(),
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: data"
            });
            println!("{}", error);
            return;
        }
    };

    let filter_str = match parsed.get("query").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: query"
            });
            println!("{}", error);
            return;
        }
    };

    let results = match apply_filter(&input_data, filter_str) {
        Ok(vals) => vals,
        Err(e) => {
            let error = json!({
                "success": false,
                "error": format!("Filter execution error: {}", e)
            });
            println!("{}", error);
            return;
        }
    };

    let result = json!({
        "success": true,
        "data": {
            "result": results
        }
    });

    println!("{}", result);
}

fn apply_filter(data: &Value, filter: &str) -> Result<Vec<Value>, String> {
    let filter = filter.trim();
    
    // Identity filter
    if filter == "." {
        return Ok(vec![data.clone()]);
    }
    
    // Array iteration .[]
    if filter == ".[]" {
        return match data {
            Value::Array(arr) => Ok(arr.clone()),
            _ => Err("Cannot iterate over non-array".to_string()),
        };
    }
    
    // Field access .field or .field1.field2
    if filter.starts_with('.') && !filter.contains('[') {
        let mut current = data.clone();
        let parts: Vec<&str> = filter[1..].split('.').collect();
        
        for part in parts {
            if part.is_empty() {
                continue;
            }
            current = match current.get(part) {
                Some(v) => v.clone(),
                None => return Ok(vec![Value::Null]),
            };
        }
        return Ok(vec![current]);
    }
    
    // Array index .[0], .[1], etc.
    if filter.starts_with(".[") && filter.ends_with(']') {
        let index_str = &filter[2..filter.len()-1];
        let index: usize = index_str.parse()
            .map_err(|_| format!("Invalid array index: {}", index_str))?;
        
        return match data {
            Value::Array(arr) => {
                Ok(vec![arr.get(index).cloned().unwrap_or(Value::Null)])
            }
            _ => Err("Cannot index non-array".to_string()),
        };
    }
    
    // Object keys
    if filter == "keys" {
        return match data {
            Value::Object(obj) => {
                let keys: Vec<Value> = obj.keys()
                    .map(|k| Value::String(k.clone()))
                    .collect();
                Ok(vec![Value::Array(keys)])
            }
            Value::Array(arr) => {
                let indices: Vec<Value> = (0..arr.len())
                    .map(|i| Value::Number(i.into()))
                    .collect();
                Ok(vec![Value::Array(indices)])
            }
            _ => Err("keys only works on objects or arrays".to_string()),
        };
    }
    
    // Length
    if filter == "length" {
        let len = match data {
            Value::Array(arr) => arr.len(),
            Value::Object(obj) => obj.keys().len(),
            Value::String(s) => s.len(),
            Value::Null => 0,
            _ => return Err("length not supported for this type".to_string()),
        };
        return Ok(vec![Value::Number(len.into())]);
    }
    
    // Type
    if filter == "type" {
        let type_name = match data {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        return Ok(vec![Value::String(type_name.to_string())]);
    }
    
    Err(format!("Unsupported filter: {}. Supported: ., .field, .[], .[n], keys, length, type", filter))
}
