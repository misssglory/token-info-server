use async_trait::async_trait;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use crate::types::{TokenInfo, ProviderError};
use crate::providers::TokenProvider;

static KNOWN_BSC_TOKENS: Lazy<HashMap<&'static str, (&'static str, u8)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d", ("USDC", 18));
    m.insert("0x55d398326f99059ff775485246999027b3197955", ("USDT", 18));
    m.insert("0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c", ("WBNB", 18));
    m.insert("0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c", ("BTCB", 18));
    m
});

pub struct BscProvider {
    priority: u8,
    rpc_url: String,
}

impl BscProvider {
    pub fn new(rpc_url: String, priority: u8) -> Self {
        tracing::info!("Initializing BSC provider with priority {}", priority);
        Self { rpc_url, priority }
    }
}

#[async_trait]
impl TokenProvider for BscProvider {
    fn name(&self) -> &str {
        "bsc-known"
    }
    
    fn chain(&self) -> &str {
        "bsc"
    }
    
    fn priority(&self) -> u8 {
        self.priority
    }
    
    fn supports_address(&self, address: &str) -> bool {
        address.starts_with("0x") && address.len() == 42 &&
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }
    
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        if let Some((symbol, decimals)) = KNOWN_BSC_TOKENS.get(address.to_lowercase().as_str()) {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(symbol.to_string()),
                decimals: *decimals,
                provider: self.name().to_string(),
            });
        }
        
        Err(ProviderError::NotFound)
    }
}