use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub items: Vec<String>,
}

#[derive(Serialize)]
pub struct Output {
    pub duplicates: Vec<String>,
    pub unique: Vec<String>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut seen = std::collections::HashSet::new();
    let mut duplicates = std::collections::HashSet::new();
    let mut unique = Vec::new();
    
    for item in input.items {
        if !seen.insert(item.clone()) {
            duplicates.insert(item);
        } else {
            unique.push(item);
        }
    }
    
    let duplicates: Vec<String> = duplicates.into_iter().collect();
    
    Ok(Output { duplicates, unique })
}
