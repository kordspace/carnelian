use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    paths: Vec<String>,
}

#[derive(Serialize)]
struct Output {
    joined: String,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn join_paths(paths: &[String]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    
    let mut result = paths[0].clone();
    
    for path in paths.iter().skip(1) {
        let path = path.trim_start_matches('/').trim_start_matches('\\');
        
        if result.ends_with('/') || result.ends_with('\\') {
            result.push_str(path);
        } else {
            result.push('/');
            result.push_str(path);
        }
    }
    
    result
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

    let joined = join_paths(&input.paths);

    let output = Output { joined };
    println!("{}", serde_json::to_string(&output).unwrap());
}
