use std::sync::Arc;
use crate::providers::TokenProvider;
use crate::types::{TokenInfo, ProviderError};
use tracing::{info, warn, debug, error};

pub async fn lookup_token(providers: &[Arc<dyn TokenProvider>], address: &str) -> TokenInfo {
    info!("Looking up token address: {}", address);
    
    // First, determine which chain this address belongs to
    let chain = detect_chain(address);
    info!("Detected chain: {}", chain);
    
    // Filter providers that support this address and belong to the detected chain
    let mut relevant_providers: Vec<&Arc<dyn TokenProvider>> = providers
        .iter()
        .filter(|p| p.supports_address(address) && p.chain() == chain)
        .collect();
    
    // Sort by priority (lower number = higher priority)
    relevant_providers.sort_by_key(|p| p.priority());
    
    if relevant_providers.is_empty() {
        warn!("No providers found for chain: {}", chain);
        return TokenInfo {
            chain: chain.to_string(),
            context: address.to_string(),
            label: None,
            decimals: 0,
            provider: "none".to_string(),
        };
    }
    
    // Try providers in priority order
    for provider in relevant_providers {
        debug!("Trying provider {} (priority {})", provider.name(), provider.priority());
        match provider.get_token_info(address).await {
            Ok(info) => {
                info!("Provider {} returned token info for {}", provider.name(), address);
                return info;
            }
            Err(e) => {
                warn!("Provider {} failed for {}: {}", provider.name(), address, e);
            }
        }
    }
    
    error!("All providers failed for address: {}", address);
    TokenInfo {
        chain: chain.to_string(),
        context: address.to_string(),
        label: None,
        decimals: 0,
        provider: "none".to_string(),
    }
}

fn detect_chain(address: &str) -> &'static str {
    if address.starts_with("0x") && address.len() == 42 {
        "ethereum" // Could also be BSC, but they use same format
    } else if address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x") {
        "solana"
    } else {
        "unknown"
    }
}