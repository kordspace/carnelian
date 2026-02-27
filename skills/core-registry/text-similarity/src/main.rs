use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Deserialize)]
struct Input {
    text1: String,
    text2: String,
    #[serde(default)]
    method: String,
}

#[derive(Serialize)]
struct Output {
    similarity: f64,
    method: String,
}

#[derive(Serialize)]
struct ErrorOutput {
    error: String,
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
    
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }
    
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();
    
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                matrix[i - 1][j - 1] + cost,
            );
        }
    }
    
    matrix[len1][len2]
}

fn jaccard_similarity(s1: &str, s2: &str) -> f64 {
    let words1: HashMap<&str, bool> = s1.split_whitespace().map(|w| (w, true)).collect();
    let words2: HashMap<&str, bool> = s2.split_whitespace().map(|w| (w, true)).collect();
    
    let intersection: usize = words1.keys().filter(|k| words2.contains_key(*k)).count();
    let union = words1.len() + words2.len() - intersection;
    
    if union == 0 {
        return 1.0;
    }
    
    intersection as f64 / union as f64
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

    let method = if input.method.is_empty() {
        "levenshtein".to_string()
    } else {
        input.method.to_lowercase()
    };

    let similarity = match method.as_str() {
        "levenshtein" => {
            let distance = levenshtein_distance(&input.text1, &input.text2);
            let max_len = std::cmp::max(input.text1.len(), input.text2.len());
            if max_len == 0 {
                1.0
            } else {
                1.0 - (distance as f64 / max_len as f64)
            }
        }
        "jaccard" => jaccard_similarity(&input.text1, &input.text2),
        _ => {
            let error = ErrorOutput {
                error: format!("Unsupported similarity method: {}", method),
            };
            println!("{}", serde_json::to_string(&error).unwrap());
            std::process::exit(1);
        }
    };

    let output = Output {
        similarity,
        method,
    };

    println!("{}", serde_json::to_string(&output).unwrap());
}
