use serde::{Deserialize, Serialize};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram: Option<String>,
}

// Add a helper method to create basic TokenInfo
impl TokenInfo {
    pub fn basic(
        chain: String,
        context: String,
        label: Option<String>,
        long_label: Option<String>,
        decimals: u8,
        provider: String,
    ) -> Self {
        Self {
            chain,
            context,
            decimals,
            label,
            long_label,
            provider,
            description: None,
            image_base64: None,
            image_url: None,
            website: None,
            twitter: None,
            telegram: None,
        }
    }
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

// TG Command for TCP endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct TgCommand {
    pub command: String,
    pub params: TgCommandParams,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TgCommandParams {
    pub mint: String,
    pub symbol: Option<String>,
    pub chain: String,
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
