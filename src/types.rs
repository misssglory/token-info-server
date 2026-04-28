use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenInfo {
    pub context: String,
    pub label: String,
    pub decimals: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),
    #[error("Invalid address format: {0}")]
    InvalidAddress(String),
    #[error("Token not found")]
    NotFound,
    #[error("Chain error: {0}")]
    ChainError(String),
}