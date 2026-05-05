use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenInfo {
    pub chain: String,
    pub context: String,
    pub decimals: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_label: Option<String>,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenSearchRequest {
    pub query: String,
    pub chain: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenSearchResult {
    pub address: String,
    pub chain: String,
    pub label: String,
    pub long_label: Option<String>,
    pub decimals: u8,
    pub provider: String,
    pub similarity: Option<f64>,
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
    #[error("Provider not configured for chain: {0}")]
    NotConfigured(String),
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub chain: String,
    pub priority: u8,
    pub enabled: bool,
}