use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    headers: Option<Vec<String>>,
    rows: Vec<Vec<String>>,
    #[serde(default)]
    delimiter: Option<char>,
}

#[derive(Serialize)]
struct Output {
    csv_data: String,
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
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(vec![]);

    if let Some(headers) = &input.headers {
        if let Err(e) = wtr.write_record(headers) {
            let error = ErrorOutput {
                error: format!("Failed to write headers: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }

    for row in &input.rows {
        if let Err(e) = wtr.write_record(row) {
            let error = ErrorOutput {
                error: format!("Failed to write row: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    }

    let csv_bytes = match wtr.into_inner() {
        Ok(b) => b,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to finalize CSV: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let csv_data = match String::from_utf8(csv_bytes) {
        Ok(s) => s,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Failed to convert CSV to UTF-8: {}", e),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        csv_data,
        row_count: input.rows.len(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
