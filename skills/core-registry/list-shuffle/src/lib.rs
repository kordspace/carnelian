use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub items: Vec<String>,
}

#[derive(Serialize)]
pub struct Output {
    pub shuffled: Vec<String>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut items = input.items;
    
    // Fisher-Yates shuffle
    let len = items.len();
    for i in (1..len).rev() {
        let j = (i as u64 * 1103515245 + 12345) % (i as u64 + 1);
        items.swap(i, j as usize);
    }
    
    Ok(Output { shuffled: items })
}
