use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub string1: String,
    pub string2: String,
}

#[derive(Serialize)]
pub struct Output {
    pub distance: usize,
    pub similarity: f64,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let s1 = input.string1.chars().collect::<Vec<_>>();
    let s2 = input.string2.chars().collect::<Vec<_>>();
    let len1 = s1.len();
    let len2 = s2.len();
    
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
    
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }
    
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1[i - 1] == s2[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }
    
    let distance = matrix[len1][len2];
    let max_len = len1.max(len2) as f64;
    let similarity = if max_len > 0.0 {
        1.0 - (distance as f64 / max_len)
    } else {
        1.0
    };
    
    Ok(Output { distance, similarity })
}
