use std::sync::Arc;
use crate::providers::TokenProvider;
use crate::types::TokenInfo;
use tracing::{info, warn, debug, error};

pub async fn lookup_token(providers: &[Arc<dyn TokenProvider>], address: &str) -> TokenInfo {
    info!("Looking up token address: {}", address);
    
    // First, determine which chain this address belongs to
    let chain = detect_chain(address);
    info!("Detected chain: {}", chain);
    
    // If chain is unknown, try to detect from address format as fallback
    let chain = if chain == "unknown" {
        if address.starts_with("0x") && address.len() == 42 {
            "ethereum"
        } else if address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x") && 
                  address.chars().all(|c| c.is_ascii_alphanumeric()) {
            "solana"
        } else {
            "unknown"
        }
    } else {
        chain
    };
    
    if chain == "unknown" {
        warn!("Could not detect chain for address: {}", address);
        return TokenInfo {
            chain: "unknown".to_string(),
            context: address.to_string(),
            label: None,
            long_label: None,
            decimals: 0,
            provider: "none".to_string(),
        };
    }
    
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
            long_label: None,
            decimals: 0,
            provider: "none".to_string(),
        };
    }
    
    // Track the best result we've found
    let mut best_result: Option<TokenInfo> = None;
    
    // Try providers in priority order
    for provider in relevant_providers {
        debug!("Trying provider {} (priority {})", provider.name(), provider.priority());
        match provider.get_token_info(address).await {
            Ok(info) => {
                // Check if this provider found a label (token name)
                if info.label.is_some() {
                    info!("Provider {} returned token WITH label: {:?}", provider.name(), info.label);
                    return info;
                } else {
                    info!("Provider {} returned token WITHOUT name, continuing search...", provider.name());
                    // Store as best result but continue searching
                    if best_result.is_none() {
                        best_result = Some(info);
                    }
                }
            }
            Err(e) => {
                warn!("Provider {} failed for {}: {}", provider.name(), address, e);
            }
        }
    }
    
    // If we found any result (even without name), return it
    if let Some(result) = best_result {
        warn!("No provider found a token name for {}, returning basic info", address);
        return result;
    }
    
    // No provider succeeded
    error!("All providers failed for address: {}", address);
    TokenInfo {
        chain: chain.to_string(),
        context: address.to_string(),
        label: None,
        long_label: None,
        decimals: 0,
        provider: "none".to_string(),
    }
}

fn detect_chain(address: &str) -> &'static str {
    if address.starts_with("0x") && address.len() == 42 {
        "ethereum" // Could also be BSC, but they use same format
    } else if address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x") &&
              address.chars().all(|c| c.is_ascii_alphanumeric()) {
        "solana"
    } else {
        "unknown"
    }
}