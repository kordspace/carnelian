use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub items: Vec<String>,
}

#[derive(Serialize)]
pub struct Output {
    pub permutations: Vec<Vec<String>>,
    pub count: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut items = input.items;
    let mut result = Vec::new();
    
    if items.len() > 8 {
        return Err("Too many items (max 8)".to_string());
    }
    
    generate_permutations(&mut items, &mut result);
    let count = result.len();
    
    Ok(Output { permutations: result, count })
}

fn generate_permutations(items: &mut [String], result: &mut Vec<Vec<String>>) {
    if items.len() <= 1 {
        result.push(items.to_vec());
        return;
    }
    
    for i in 0..items.len() {
        items.swap(0, i);
        let mut rest: Vec<String> = items[1..].to_vec();
        let mut sub_perms = Vec::new();
        generate_permutations(&mut rest, &mut sub_perms);
        
        for perm in sub_perms {
            let mut full_perm = vec![items[0].clone()];
            full_perm.extend(perm);
            result.push(full_perm);
        }
        items.swap(0, i);
    }
}
