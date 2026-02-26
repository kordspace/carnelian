use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use url::Url;

#[derive(Deserialize)]
struct Input {
    url: String,
}

#[derive(Serialize)]
struct Output {
    scheme: String,
    host: Option<String>,
    port: Option<u16>,
    path: String,
    query: Option<HashMap<String, String>>,
    fragment: Option<String>,
    username: String,
    password: Option<String>,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
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

    let parsed = match Url::parse(&input.url) {
        Ok(u) => u,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to parse URL: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let query: Option<HashMap<String, String>> = if parsed.query().is_some() {
        let mut map = HashMap::new();
        for (key, value) in parsed.query_pairs() {
            map.insert(key.to_string(), value.to_string());
        }
        Some(map)
    } else {
        None
    };

    let output = Output {
        scheme: parsed.scheme().to_string(),
        host: parsed.host_str().map(|s| s.to_string()),
        port: parsed.port(),
        path: parsed.path().to_string(),
        query,
        fragment: parsed.fragment().map(|s| s.to_string()),
        username: parsed.username().to_string(),
        password: parsed.password().map(|s| s.to_string()),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
