use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub text: String,
    pub pattern: String,
}

#[derive(Serialize)]
pub struct Output {
    pub matches: Vec<Match>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub matched: String,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut matches = Vec::new();
    let text = input.text;
    let pattern = input.pattern;
    
    if pattern.is_empty() {
        return Ok(Output { matches, count: 0 });
    }
    
    let mut start = 0;
    while let Some(pos) = text[start..].find(&pattern) {
        let match_start = start + pos;
        let match_end = match_start + pattern.len();
        matches.push(Match {
            start: match_start,
            end: match_end,
            matched: text[match_start..match_end].to_string(),
        });
        start = match_start + 1;
    }
    
    let count = matches.len();
    Ok(Output { matches, count })
}
