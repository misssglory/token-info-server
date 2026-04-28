// src/config.rs
use std::env;

pub struct Config {
    pub solana_rpc: String,
    pub ethereum_rpc: String,
    pub bsc_rpc: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            solana_rpc: env::var("SOLANA_RPC").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            ethereum_rpc: env::var("ETHEREUM_RPC").unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string()),
            bsc_rpc: env::var("BSC_RPC").unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string()),
        }
    }
}