use std::env;
use std::collections::HashMap;
use crate::types::ProviderConfig;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Deserialize)]
pub struct RpcConfig {
    pub solana: String,
    pub ethereum: String,
    pub bsc: String,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub rpc: RpcConfig,
}

impl AppConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&contents)?;
        Ok(config)
    }
    
    pub fn from_env() -> Self {
        // Fallback to environment variables if TOML not found
        Self {
            server: ServerConfig {
                host: env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080),
            },
            logging: LoggingConfig {
                level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            },
            rpc: RpcConfig {
                solana: env::var("SOLANA_RPC").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
                ethereum: env::var("ETHEREUM_RPC").unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string()),
                bsc: env::var("BSC_RPC").unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string()),
            },
        }
    }
}

// Keep original Config struct for backward compatibility
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
                name: "metadata-api".to_string(),
                chain: "solana".to_string(),
                priority: 1,
                enabled: true,
            },
            ProviderConfig {
                name: "pumpfun".to_string(),
                chain: "solana".to_string(),
                priority: 2,
                enabled: true,
            },
            ProviderConfig {
                name: "helius".to_string(),
                chain: "solana".to_string(),
                priority: 3,
                enabled: helius_api_key.is_some(),
            },
            ProviderConfig {
                name: "solana-rpc".to_string(),
                chain: "solana".to_string(),
                priority: 4,
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