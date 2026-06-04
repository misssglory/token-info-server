use crate::providers::TokenProvider;
use crate::types::{ProviderError, TokenInfo};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

pub struct HeliusProvider {
    api_key: String,
    http_client: Client,
    priority: u8,
}

impl HeliusProvider {
    pub fn new(api_key: String, priority: u8) -> Self {
        tracing::info!("Initializing Helius provider with priority {}", priority);
        Self {
            api_key,
            http_client: Client::new(),
            priority,
        }
    }
    
    pub async fn get_token_metadata(&self, mint: &str) -> Option<TokenInfo> {
        let url = format!("https://api.helius.xyz/v0/tokens/metadata?api-key={}", self.api_key);
        
        let request = json!({
            "mintAccounts": [mint]
        });
        
        match self.http_client.post(&url).json(&request).send().await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(metadata) => {
                        if let Some(result) = metadata.as_array().and_then(|arr| arr.get(0)) {
                            let off_chain = match result.get("offChain") {
                                Some(oc) => oc,
                                None => return None,
                            };
                            
                            let symbol = off_chain.get("symbol")
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string());
                            
                            let name = off_chain.get("name")
                                .and_then(|n| n.as_str())
                                .map(|n| n.to_string());
                            
                            let description = off_chain.get("description")
                                .and_then(|d| d.as_str())
                                .map(|d| d.to_string());
                            
                            let image_url = off_chain.get("image")
                                .and_then(|i| i.as_str())
                                .map(|i| i.to_string());
                            
                            let website = off_chain.get("website")
                                .and_then(|w| w.as_str())
                                .map(|w| w.to_string());
                            
                            let twitter = off_chain.get("twitter")
                                .and_then(|t| t.as_str())
                                .map(|t| t.to_string());
                            
                            let telegram = off_chain.get("telegram")
                                .and_then(|t| t.as_str())
                                .map(|t| t.to_string());
                            
                            let decimals = result.get("onChain")
                                .and_then(|oc| oc.get("decimals"))
                                .and_then(|d| d.as_u64())
                                .unwrap_or(6) as u8;
                            
                            return Some(TokenInfo {
                                chain: "solana".to_string(),
                                context: mint.to_string(),
                                label: symbol,
                                long_label: name,
                                description,
                                image_base64: None,
                                image_url,
                                website,
                                twitter,
                                telegram,
                                decimals,
                                provider: "helius".to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Failed to parse Helius response JSON: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to fetch from Helius: {}", e);
            }
        }
        
        None
    }
}

#[async_trait]
impl TokenProvider for HeliusProvider {
    fn name(&self) -> &str {
        "helius"
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
        if let Some(info) = self.get_token_metadata(address).await {
            return Ok(info);
        }
        
        Err(ProviderError::NotFound)
    }
}