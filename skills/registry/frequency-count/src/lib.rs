use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub items: Vec<String>,
}

#[derive(Serialize)]
pub struct Output {
    pub frequency: std::collections::HashMap<String, usize>,
    pub most_common: Option<(String, usize)>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut frequency = std::collections::HashMap::new();
    
    for item in input.items {
        *frequency.entry(item).or_insert(0) += 1;
    }
    
    let most_common = frequency.iter()
        .max_by_key(|(_, count)| *count)
        .map(|(item, count)| (item.clone(), *count));
    
    Ok(Output { frequency, most_common })
}
