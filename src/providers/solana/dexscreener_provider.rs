use crate::providers::TokenProvider;
use crate::types::{ProviderError, TokenInfo};
use async_trait::async_trait;
use reqwest::Client;

pub struct DexScreenerProvider {
    priority: u8,
    http_client: Client,
}

impl DexScreenerProvider {
    pub fn new(priority: u8) -> Self {
        tracing::info!("Initializing DexScreener provider with priority {}", priority);
        Self {
            priority,
            http_client: Client::new(),
        }
    }
    
    async fn fetch_dexscreener_info(&self, address: &str) -> Option<(String, String, Option<String>)> {
        let url = format!("https://api.dexscreener.com/latest/dex/token/{}", address);
        
        match self.http_client.get(&url).send().await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if let Some(pairs) = json.get("pairs").and_then(|p| p.as_array()) {
                            if let Some(first_pair) = pairs.first() {
                                let base_token = first_pair.get("baseToken")?;
                                
                                let name = base_token.get("name")?.as_str()?.to_string();
                                let symbol = base_token.get("symbol")?.as_str()?.to_string();
                                let description = first_pair.get("info")
                                    .and_then(|i| i.get("description"))
                                    .and_then(|d| d.as_str())
                                    .map(|s| s.to_string());
                                
                                return Some((name, symbol, description));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Failed to parse DexScreener response: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to fetch from DexScreener: {}", e);
            }
        }
        None
    }
}

#[async_trait]
impl TokenProvider for DexScreenerProvider {
    fn name(&self) -> &str {
        "dexscreener"
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
        if let Some((name, symbol, description)) = self.fetch_dexscreener_info(address).await {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(symbol),
                long_label: Some(name),
                description,
                image_base64: None,
                image_url: None,
                website: None,
                twitter: None,
                telegram: None,
                decimals: 6, // Default, will be overridden by other providers
                provider: self.name().to_string(),
            });
        }
        
        Err(ProviderError::NotFound)
    }
}