//! Configuration management

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub http_port: u16,
    pub ws_port: u16,
    pub ollama_url: String,
    pub machine_profile: MachineProfile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MachineProfile {
    Thummim, // 2080 Super, 32GB RAM (constrained)
    Urim,    // 2080 Ti, 64GB RAM (high-end)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgresql://carnelian:carnelian@localhost:5432/carnelian".to_string(),
            http_port: 3000,
            ws_port: 3001,
            ollama_url: "http://localhost:11434".to_string(),
            machine_profile: MachineProfile::Thummim,
        }
    }
}
