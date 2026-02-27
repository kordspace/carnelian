use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    csv_data: String,
    #[serde(default)]
    delimiter: Option<char>,
    #[serde(default = "default_true")]
    has_headers: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Output {
    headers: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
    row_count: usize,
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

    let delimiter = input.delimiter.unwrap_or(',') as u8;
    
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(input.has_headers)
        .from_reader(input.csv_data.as_bytes());

    let headers = if input.has_headers {
        match reader.headers() {
            Ok(h) => Some(h.iter().map(|s| s.to_string()).collect()),
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Failed to read headers: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    let mut rows = Vec::new();
    for result in reader.records() {
        match result {
            Ok(record) => {
                let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                rows.push(row);
            }
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Failed to parse CSV row: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        }
    }

    let output = Output {
        headers,
        row_count: rows.len(),
        rows,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
