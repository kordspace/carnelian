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

    let text = match parsed.get("text").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: text"
            });
            println!("{}", error);
            return;
        }
    };

    let pattern = match parsed.get("pattern").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            let error = json!({
                "success": false,
                "error": "Missing required field: pattern"
            });
            println!("{}", error);
            return;
        }
    };

    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            let error = json!({
                "success": false,
                "error": format!("Invalid regex pattern: {}", e)
            });
            println!("{}", error);
            return;
        }
    };

    let mut matches = Vec::new();
    for cap in re.captures_iter(text) {
        let m = cap.get(0).unwrap();
        let mut match_obj = json!({
            "match": m.as_str(),
            "start": m.start(),
            "end": m.end()
        });

        let mut groups = Vec::new();
        for i in 1..cap.len() {
            if let Some(group) = cap.get(i) {
                groups.push(json!({
                    "match": group.as_str(),
                    "start": group.start(),
                    "end": group.end()
                }));
            } else {
                groups.push(Value::Null);
            }
        }

        if !groups.is_empty() {
            match_obj.as_object_mut().unwrap().insert("groups".to_string(), json!(groups));
        }

        matches.push(match_obj);
    }

    let result = json!({
        "success": true,
        "data": {
            "matches": matches,
            "count": matches.len()
        }
    });

    println!("{}", result);
}
