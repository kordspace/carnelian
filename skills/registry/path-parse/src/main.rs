use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    path: String,
}

#[derive(Serialize)]
struct Output {
    dir: String,
    base: String,
    ext: String,
    name: String,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn parse_path(path: &str) -> (String, String, String, String) {
    let path = path.replace('\\', "/");
    
    let last_slash = path.rfind('/');
    let (dir, base) = if let Some(pos) = last_slash {
        (path[..pos].to_string(), path[pos + 1..].to_string())
    } else {
        (String::new(), path.clone())
    };
    
    let last_dot = base.rfind('.');
    let (name, ext) = if let Some(pos) = last_dot {
        if pos > 0 {
            (base[..pos].to_string(), base[pos..].to_string())
        } else {
            (base.clone(), String::new())
        }
    } else {
        (base.clone(), String::new())
    };
    
    (dir, base, ext, name)
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

    let (dir, base, ext, name) = parse_path(&input.path);

    let output = Output {
        dir,
        base,
        ext,
        name,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
