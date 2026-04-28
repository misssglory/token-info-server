use async_trait::async_trait;
use crate::types::{TokenInfo, ProviderError};
use crate::providers::TokenProvider;
use reqwest::Client;
use serde_json::json;

pub struct MetadataApiProvider {
    http_client: Client,
    priority: u8,
}

impl MetadataApiProvider {
    pub fn new(priority: u8) -> Self {
        tracing::info!("Initializing Metadata API provider with priority {}", priority);
        Self {
            http_client: Client::new(),
            priority,
        }
    }
    
    async fn fetch_from_tx_parser(&self, mint_address: &str) -> Option<(String, u8)> {
        // Using tx.parser API (public endpoint)
        let url = format!("https://api.tx.parser.solana.com/token/{}", mint_address);
        
        if let Ok(response) = self.http_client.get(&url).send().await {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                    let symbol = json.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
                    let decimals = json.get("decimals").and_then(|d| d.as_u64()).unwrap_or(6) as u8;
                    return Some((format!("{} ({})", name, symbol), decimals));
                }
            }
        }
        None
    }
    
    async fn fetch_from_das_api(&self, mint_address: &str) -> Option<(String, u8)> {
        // Using Helius DAS API (if you have API key)
        if let Ok(api_key) = std::env::var("HELIUS_API_KEY") {
            let url = format!("https://mainnet.helius-rpc.com/?api-key={}", api_key);
            let body = json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "getAsset",
                "params": [mint_address]
            });
            
            if let Ok(response) = self.http_client.post(&url).json(&body).send().await {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if let Some(content) = json.get("result").and_then(|r| r.get("content")) {
                        if let Some(metadata) = content.get("metadata") {
                            if let Some(name) = metadata.get("name").and_then(|n| n.as_str()) {
                                let symbol = metadata.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
                                return Some((format!("{} ({})", name, symbol), 6));
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[async_trait]
impl TokenProvider for MetadataApiProvider {
    fn name(&self) -> &str {
        "metadata-api"
    }
    
    fn chain(&self) -> &str {
        "solana"
    }
    
    fn priority(&self) -> u8 {
        self.priority
    }
    
    fn supports_address(&self, address: &str) -> bool {
        address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x")
    }
    
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        if let Some((name, decimals)) = self.fetch_from_tx_parser(address).await {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(name),
                decimals,
                provider: self.name().to_string(),
            });
        }
        
        Err(ProviderError::NotFound)
    }
}