use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
}

#[derive(Serialize)]
pub struct Output {
    pub word_count: usize,
    pub char_count: usize,
    pub char_count_no_spaces: usize,
    pub line_count: usize,
    pub sentence_count: usize,
    pub paragraph_count: usize,
    pub avg_word_length: f64,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let text = &input.text;
    
    let char_count = text.chars().count();
    let char_count_no_spaces = text.chars().filter(|&c| !c.is_whitespace()).count();
    
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();
    
    let line_count = text.lines().count();
    let line_count = if line_count == 0 && !text.is_empty() { 1 } else { line_count };
    
    let sentence_count = text.split(|c| c == '.' || c == '!' || c == '?').filter(|s| !s.trim().is_empty()).count();
    let sentence_count = if sentence_count == 0 && !text.is_empty() { 1 } else { sentence_count };
    
    let paragraph_count = text.split("\n\n").filter(|p| !p.trim().is_empty()).count();
    let paragraph_count = if paragraph_count == 0 && !text.is_empty() { 1 } else { paragraph_count };
    
    let total_word_length: usize = words.iter().map(|w| w.chars().count()).sum();
    let avg_word_length = if word_count > 0 {
        total_word_length as f64 / word_count as f64
    } else {
        0.0
    };
    
    Ok(Output {
        word_count,
        char_count,
        char_count_no_spaces,
        line_count,
        sentence_count,
        paragraph_count,
        avg_word_length,
    })
}
