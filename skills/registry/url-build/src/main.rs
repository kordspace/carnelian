use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use url::Url;

#[derive(Deserialize)]
struct Input {
    scheme: String,
    host: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    query: Option<HashMap<String, String>>,
    #[serde(default)]
    fragment: Option<String>,
}

fn default_path() -> String {
    "/".to_string()
}

#[derive(Serialize)]
struct Output {
    url: String,
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

    let base = if let Some(port) = input.port {
        format!("{}://{}:{}", input.scheme, input.host, port)
    } else {
        format!("{}://{}", input.scheme, input.host)
    };

    let mut url = match Url::parse(&base) {
        Ok(u) => u,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to build base URL: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    url.set_path(&input.path);

    if let Some(query_params) = input.query {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_params {
            query.append_pair(&key, &value);
        }
    }

    if let Some(fragment) = input.fragment {
        url.set_fragment(Some(&fragment));
    }

    let output = Output {
        url: url.to_string(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
