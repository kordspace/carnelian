use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub list1: Vec<String>,
    pub list2: Vec<String>,
}

#[derive(Serialize)]
pub struct Output {
    pub intersection: Vec<String>,
    pub union: Vec<String>,
    pub difference: Vec<String>,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let set1: std::collections::HashSet<String> = input.list1.into_iter().collect();
    let set2: std::collections::HashSet<String> = input.list2.into_iter().collect();
    
    let intersection: Vec<String> = set1.intersection(&set2).cloned().collect();
    let union: Vec<String> = set1.union(&set2).cloned().collect();
    let difference: Vec<String> = set1.difference(&set2).cloned().collect();
    
    Ok(Output { intersection, union, difference })
}
