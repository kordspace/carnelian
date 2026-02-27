use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub items: Vec<String>,
    pub k: usize,
}

#[derive(Serialize)]
pub struct Output {
    pub combinations: Vec<Vec<String>>,
    pub count: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let items = input.items;
    let k = input.k;
    
    if k > items.len() {
        return Err("k cannot be larger than item count".to_string());
    }
    
    let mut result = Vec::new();
    let mut current = Vec::with_capacity(k);
    
    generate_combinations(&items, k, 0, &mut current, &mut result);
    let count = result.len();
    
    Ok(Output { combinations: result, count })
}

fn generate_combinations(
    items: &[String],
    k: usize,
    start: usize,
    current: &mut Vec<String>,
    result: &mut Vec<Vec<String>>,
) {
    if current.len() == k {
        result.push(current.clone());
        return;
    }
    
    for i in start..items.len() {
        current.push(items[i].clone());
        generate_combinations(items, k, i + 1, current, result);
        current.pop();
    }
}
