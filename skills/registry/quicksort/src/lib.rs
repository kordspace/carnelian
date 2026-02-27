use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub numbers: Vec<i64>,
}

#[derive(Serialize)]
pub struct Output {
    pub sorted: Vec<i64>,
    pub comparisons: usize,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let mut arr = input.numbers;
    let mut comparisons = 0;
    
    quicksort(&mut arr, &mut comparisons);
    
    Ok(Output { sorted: arr, comparisons })
}

fn quicksort(arr: &mut [i64], comparisons: &mut usize) {
    if arr.len() <= 1 {
        return;
    }
    
    let pivot_index = partition(arr, comparisons);
    let len = arr.len();
    
    quicksort(&mut arr[0..pivot_index], comparisons);
    quicksort(&mut arr[pivot_index + 1..len], comparisons);
}

fn partition(arr: &mut [i64], comparisons: &mut usize) -> usize {
    let len = arr.len();
    let pivot = arr[len - 1];
    let mut i = 0;
    
    for j in 0..len - 1 {
        *comparisons += 1;
        if arr[j] <= pivot {
            arr.swap(i, j);
            i += 1;
        }
    }
    
    arr.swap(i, len - 1);
    i
}
