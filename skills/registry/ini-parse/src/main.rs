use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    ini_data: String,
}

#[derive(Serialize)]
struct Output {
    data: HashMap<String, HashMap<String, String>>,
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

    let mut config = configparser::ini::Ini::new();
    
    if let Err(e) = config.read(input.ini_data) {
        let error = ErrorOutput {
            error: format!("Failed to parse INI: {}", e),
        };
        println!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }

    let raw_data = config.get_map().unwrap_or_default();
    
    let data: HashMap<String, HashMap<String, String>> = raw_data
        .into_iter()
        .map(|(section, props)| {
            let converted_props: HashMap<String, String> = props
                .into_iter()
                .filter_map(|(k, v)| v.map(|val| (k, val)))
                .collect();
            (section, converted_props)
        })
        .collect();

    let output = Output { data };
    println!("{}", serde_json::to_string(&output).unwrap());
}
