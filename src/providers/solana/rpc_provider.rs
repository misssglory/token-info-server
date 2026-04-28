use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Mint;
use spl_token::solana_program::program_pack::Pack;
use std::str::FromStr;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use crate::types::{TokenInfo, ProviderError};
use crate::providers::TokenProvider;

static KNOWN_TOKENS: Lazy<HashMap<&'static str, (&'static str, u8)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", ("USDC", 6));
    m.insert("So11111111111111111111111111111111111111112", ("Wrapped SOL", 9));
    m.insert("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", ("USDT", 6));
    m.insert("mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So", ("mSOL", 9));
    m
});

pub struct SolanaRpcProvider {
    rpc_client: RpcClient,
    priority: u8,
}

impl SolanaRpcProvider {
    pub fn new(rpc_url: String, priority: u8) -> Self {
        tracing::info!("Initializing Solana RPC provider with priority {}", priority);
        Self {
            rpc_client: RpcClient::new(rpc_url),
            priority,
        }
    }
}

#[async_trait]
impl TokenProvider for SolanaRpcProvider {
    fn name(&self) -> &str {
        "solana-rpc"
    }
    
    fn chain(&self) -> &str {
        "solana"
    }
    
    fn priority(&self) -> u8 {
        self.priority
    }
    
    fn supports_address(&self, address: &str) -> bool {
        address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x") && 
        address.chars().all(|c| c.is_ascii_alphanumeric())
    }
    
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        // Check known tokens first
        if let Some((name, decimals)) = KNOWN_TOKENS.get(address) {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(name.to_string()),
                decimals: *decimals,
                provider: self.name().to_string(),
            });
        }
        
        let pubkey = Pubkey::from_str(address)
            .map_err(|_| ProviderError::InvalidAddress(address.to_string()))?;
        
        let mint_account = self.rpc_client.get_account(&pubkey)
            .map_err(|_| ProviderError::NotFound)?;
        
        let mint = Mint::unpack(&mint_account.data)
            .map_err(|_| ProviderError::ChainError("Invalid mint account".to_string()))?;
        
        Ok(TokenInfo {
            chain: self.chain().to_string(),
            context: address.to_string(),
            label: None,
            decimals: mint.decimals,
            provider: self.name().to_string(),
        })
    }
}