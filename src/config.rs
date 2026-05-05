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

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: u32,
    pub cache_ttl_seconds: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TcpEndpointConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub forward_tokens: bool,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub rpc: RpcConfig,
    pub database: DatabaseConfig,
    pub tcp_endpoint: TcpEndpointConfig,
}

impl AppConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&contents)?;
        Ok(config)
    }
    
    pub fn from_env() -> Self {
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
            database: DatabaseConfig {
                host: env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string()).parse().unwrap_or(5432),
                database: env::var("DB_NAME").unwrap_or_else(|_| "token_cache".to_string()),
                username: env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string()),
                password: env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string()),
                max_connections: env::var("DB_MAX_CONNECTIONS").unwrap_or_else(|_| "10".to_string()).parse().unwrap_or(10),
                cache_ttl_seconds: env::var("CACHE_TTL_SECONDS").unwrap_or_else(|_| "86400".to_string()).parse().unwrap_or(86400),
            },
            tcp_endpoint: TcpEndpointConfig {
                enabled: env::var("TCP_ENDPOINT_ENABLED").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true),
                host: env::var("TCP_ENDPOINT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("TCP_ENDPOINT_PORT").unwrap_or_else(|_| "9003".to_string()).parse().unwrap_or(9003),
                forward_tokens: env::var("TCP_FORWARD_TOKENS").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true),
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
        
        let mut provider_priorities = HashMap::new();
        
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