use std::io::{self, Read, Write};

fn main() {
    // Read all of stdin
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        let error_response = serde_json::json!({
            "success": false,
            "error": format!("Failed to read stdin: {}", e)
        });
        println!("{}", error_response);
        return;
    }

    // Try to parse as JSON
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&input);
    
    let response = match parsed {
        Ok(data) => {
            serde_json::json!({
                "success": true,
                "data": {
                    "message": "Hello from WASM!",
                    "echo": data
                }
            })
        }
        Err(_) => {
            serde_json::json!({
                "success": true,
                "data": {
                    "message": "Hello from WASM!",
                    "raw": input.trim()
                }
            })
        }
    };

    // Write response to stdout
    if let Err(e) = writeln!(io::stdout(), "{}", response) {
        eprintln!("Failed to write stdout: {}", e);
    }
}
