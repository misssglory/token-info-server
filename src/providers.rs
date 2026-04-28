use async_trait::async_trait;
use crate::types::{TokenInfo, ProviderError};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Mint;
use spl_token::solana_program::program_pack::Pack;  // Add this import
use std::str::FromStr;
use std::env;
use std::collections::HashMap;
use once_cell::sync::Lazy;

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn supports_address(&self, address: &str) -> bool;
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError>;
}

// Known token database
static KNOWN_TOKENS: Lazy<HashMap<&'static str, (&'static str, u8)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // Solana Mainnet tokens
    m.insert("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", ("USDC", 6));
    m.insert("So11111111111111111111111111111111111111112", ("Wrapped SOL", 9));
    m.insert("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", ("USDT", 6));
    m.insert("mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So", ("mSOL", 9));
    m.insert("JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN", ("JUP", 9));
    m.insert("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263", ("BONK", 5));
    m
});

pub struct SolanaProvider {
    rpc_client: RpcClient,
}

impl SolanaProvider {
    pub fn new() -> Self {
        let rpc_url = env::var("SOLANA_RPC")
            .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
        tracing::info!("Initializing Solana provider with RPC: {}", rpc_url);
        Self {
            rpc_client: RpcClient::new(rpc_url),
        }
    }
    
    fn get_known_token_info(&self, address: &str) -> Option<(String, u8)> {
        KNOWN_TOKENS.get(address).map(|(name, decimals)| {
            (name.to_string(), *decimals)
        })
    }
}

#[async_trait]
impl Provider for SolanaProvider {
    fn name(&self) -> &str {
        "Solana"
    }

    fn supports_address(&self, address: &str) -> bool {
        // Solana addresses are base58 encoded and typically 32-44 chars
        address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x") && 
        address.chars().all(|c| c.is_ascii_alphanumeric())
    }

    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        tracing::info!("Fetching Solana token info for: {}", address);
        
        // Check known tokens first (fast path)
        if let Some((name, decimals)) = self.get_known_token_info(address) {
            tracing::info!("Found known token: {} with {} decimals", name, decimals);
            return Ok(TokenInfo {
                context: format!("solana:{}", address),
                label: name,
                decimals,
            });
        }
        
        // Parse the address
        let pubkey = Pubkey::from_str(address)
            .map_err(|_| ProviderError::InvalidAddress(address.to_string()))?;
        
        // Get token mint account info
        let mint_account = self.rpc_client.get_account(&pubkey)
            .map_err(|e| {
                tracing::error!("Failed to get mint account: {}", e);
                ProviderError::NotFound
            })?;
        
        // Unpack mint data using the Pack trait's unpack method
        let mint = Mint::unpack(&mint_account.data)
            .map_err(|_| ProviderError::ChainError("Invalid mint account".to_string()))?;
        
        // Generate a label from the address if unknown
        let label = format!("{}...{}", &address[..4], &address[address.len()-4..]);
        
        tracing::info!("Successfully fetched token with {} decimals", mint.decimals);
        
        Ok(TokenInfo {
            context: format!("solana:{}", address),
            label,
            decimals: mint.decimals,
        })
    }
}

pub struct EthereumProvider {
    rpc_url: String,
}

impl EthereumProvider {
    pub fn new() -> Self {
        let rpc_url = env::var("ETHEREUM_RPC")
            .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string());
        Self { rpc_url }
    }
}

#[async_trait]
impl Provider for EthereumProvider {
    fn name(&self) -> &str {
        "Ethereum"
    }

    fn supports_address(&self, address: &str) -> bool {
        address.starts_with("0x") && address.len() == 42 && 
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        tracing::info!("Fetching Ethereum token info for: {}", address);
        
        // Known Ethereum tokens
        let label = match address.to_lowercase().as_str() {
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => "USDC".to_string(),
            "0xdac17f958d2ee523a2206206994597c13d831ec7" => "USDT".to_string(),
            "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => "WBTC".to_string(),
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" => "WETH".to_string(),
            _ => format!("{}...{}", &address[..6], &address[address.len()-4..]),
        };
        
        Ok(TokenInfo {
            context: format!("ethereum:{}", address),
            label,
            decimals: 18,
        })
    }
}

pub struct BscProvider {
    rpc_url: String,
}

impl BscProvider {
    pub fn new() -> Self {
        let rpc_url = env::var("BSC_RPC")
            .unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string());
        Self { rpc_url }
    }
}

#[async_trait]
impl Provider for BscProvider {
    fn name(&self) -> &str {
        "BSC"
    }

    fn supports_address(&self, address: &str) -> bool {
        address.starts_with("0x") && address.len() == 42 &&
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        tracing::info!("Fetching BSC token info for: {}", address);
        
        // Known BSC tokens
        let label = match address.to_lowercase().as_str() {
            "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d" => "USDC".to_string(),
            "0x55d398326f99059ff775485246999027b3197955" => "USDT".to_string(),
            "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c" => "WBNB".to_string(),
            "0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c" => "BTCB".to_string(),
            _ => format!("{}...{}", &address[..6], &address[address.len()-4..]),
        };
        
        Ok(TokenInfo {
            context: format!("bsc:{}", address),
            label,
            decimals: 18,
        })
    }
}

impl Default for SolanaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for EthereumProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for BscProvider {
    fn default() -> Self {
        Self::new()
    }
}