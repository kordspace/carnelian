use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Input {
    pub data: Vec<u8>,
}

#[derive(Serialize)]
pub struct Output {
    pub checksum: u32,
}

pub fn execute(input: Input) -> Result<Output, String> {
    let checksum = crc32fast::hash(&input.data);
    Ok(Output { checksum })
}
