use async_trait::async_trait;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use crate::types::{TokenInfo, ProviderError};
use crate::providers::TokenProvider;

static KNOWN_ETH_TOKENS: Lazy<HashMap<&'static str, (&'static str, u8)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", ("USDC", 6));
    m.insert("0xdac17f958d2ee523a2206206994597c13d831ec7", ("USDT", 6));
    m.insert("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", ("WBTC", 8));
    m.insert("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", ("WETH", 18));
    m.insert("0x6b175474e89094c44da98b954eedeac495271d0f", ("DAI", 18));
    m
});

pub struct EthereumProvider {
    priority: u8,
    rpc_url: String,
}

impl EthereumProvider {
    pub fn new(rpc_url: String, priority: u8) -> Self {
        tracing::info!("Initializing Ethereum provider with priority {}", priority);
        Self { rpc_url, priority }
    }
}

#[async_trait]
impl TokenProvider for EthereumProvider {
    fn name(&self) -> &str {
        "ethereum-known"
    }
    
    fn chain(&self) -> &str {
        "ethereum"
    }
    
    fn priority(&self) -> u8 {
        self.priority
    }
    
    fn supports_address(&self, address: &str) -> bool {
        address.starts_with("0x") && address.len() == 42 && 
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }
    
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        if let Some((symbol, decimals)) = KNOWN_ETH_TOKENS.get(address.to_lowercase().as_str()) {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(symbol.to_string()),
                long_label: None,
                decimals: *decimals,
                provider: self.name().to_string(),
            });
        }
        
        Err(ProviderError::NotFound)
    }
}
