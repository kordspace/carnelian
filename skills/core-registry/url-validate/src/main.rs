use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    url: String,
}

#[derive(Serialize)]
struct Output {
    valid: bool,
    protocol: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    path: Option<String>,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn validate_url(url: &str) -> (bool, Option<String>, Option<String>, Option<u16>, Option<String>) {
    if url.is_empty() {
        return (false, None, None, None, None);
    }

    let protocol_end = url.find("://");
    if protocol_end.is_none() {
        return (false, None, None, None, None);
    }

    let protocol_end = protocol_end.unwrap();
    let protocol = url[..protocol_end].to_lowercase();
    
    if !matches!(protocol.as_str(), "http" | "https" | "ftp" | "ws" | "wss") {
        return (false, None, None, None, None);
    }

    let after_protocol = &url[protocol_end + 3..];
    
    let path_start = after_protocol.find('/').unwrap_or(after_protocol.len());
    let host_port = &after_protocol[..path_start];
    let path = if path_start < after_protocol.len() {
        Some(after_protocol[path_start..].to_string())
    } else {
        None
    };

    let (host, port) = if let Some(colon_pos) = host_port.rfind(':') {
        let h = host_port[..colon_pos].to_string();
        let p = host_port[colon_pos + 1..].parse::<u16>().ok();
        (h, p)
    } else {
        (host_port.to_string(), None)
    };

    if host.is_empty() {
        return (false, None, None, None, None);
    }

    (true, Some(protocol), Some(host), port, path)
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

    let (valid, protocol, host, port, path) = validate_url(&input.url);

    let output = Output {
        valid,
        protocol,
        host,
        port,
        path,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
