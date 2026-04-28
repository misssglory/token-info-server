use std::env;
use std::collections::HashMap;
use crate::types::ProviderConfig;

pub struct Config {
    pub solana_rpc: String,
    pub ethereum_rpc: String,
    pub bsc_rpc: String,
    pub helius_api_key: Option<String>,
    pub provider_priorities: HashMap<String, Vec<ProviderConfig>>,
}

impl Config {
    pub fn from_env() -> Self {
        let helius_api_key = env::var("HELIUS_API_KEY").ok();
        
        // Define provider priorities for each chain
        let mut provider_priorities = HashMap::new();
        
        // Solana chain providers (higher priority = lower number)
        provider_priorities.insert("solana".to_string(), vec![
            ProviderConfig {
                name: "pumpfun".to_string(),
                chain: "solana".to_string(),
                priority: 1,
                enabled: true,
            },
            ProviderConfig {
                name: "helius".to_string(),
                chain: "solana".to_string(),
                priority: 2,
                enabled: helius_api_key.is_some(),
            },
            ProviderConfig {
                name: "solana-rpc".to_string(),
                chain: "solana".to_string(),
                priority: 3,
                enabled: true,
            },
        ]);
        
        // Ethereum chain providers
        provider_priorities.insert("ethereum".to_string(), vec![
            ProviderConfig {
                name: "ethereum-known".to_string(),
                chain: "ethereum".to_string(),
                priority: 1,
                enabled: true,
            },
            ProviderConfig {
                name: "ethereum-rpc".to_string(),
                chain: "ethereum".to_string(),
                priority: 2,
                enabled: true,
            },
        ]);
        
        // BSC chain providers
        provider_priorities.insert("bsc".to_string(), vec![
            ProviderConfig {
                name: "bsc-known".to_string(),
                chain: "bsc".to_string(),
                priority: 1,
                enabled: true,
            },
            ProviderConfig {
                name: "bsc-rpc".to_string(),
                chain: "bsc".to_string(),
                priority: 2,
                enabled: true,
            },
        ]);
        
        Self {
            solana_rpc: env::var("SOLANA_RPC").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            ethereum_rpc: env::var("ETHEREUM_RPC").unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string()),
            bsc_rpc: env::var("BSC_RPC").unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string()),
            helius_api_key,
            provider_priorities,
        }
    }
    
    pub fn get_providers_for_chain(&self, chain: &str) -> Vec<ProviderConfig> {
        self.provider_priorities
            .get(chain)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.enabled)
            .collect()
    }
}