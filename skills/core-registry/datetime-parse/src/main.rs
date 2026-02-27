use chrono::{DateTime, Datelike, NaiveDateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    datetime: String,
    #[serde(default)]
    format: Option<String>,
}

#[derive(Serialize)]
struct Output {
    timestamp: i64,
    iso8601: String,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
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

    let dt: DateTime<Utc> = if let Some(fmt) = input.format {
        match NaiveDateTime::parse_from_str(&input.datetime, &fmt) {
            Ok(naive) => DateTime::from_naive_utc_and_offset(naive, Utc),
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Failed to parse datetime with format '{}': {}", fmt, e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        }
    } else {
        match input.datetime.parse::<DateTime<Utc>>() {
            Ok(dt) => dt,
            Err(e) => {
                let error = ErrorOutput {
                    error: format!("Failed to parse datetime: {}", e),
                };
                println!("{}", serde_json::to_string(&error).unwrap());
                std::process::exit(1);
            }
        }
    };

    let output = Output {
        timestamp: dt.timestamp(),
        iso8601: dt.to_rfc3339(),
        year: dt.year(),
        month: dt.month(),
        day: dt.day(),
        hour: dt.hour(),
        minute: dt.minute(),
        second: dt.second(),
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
