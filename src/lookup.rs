use std::sync::Arc;
use crate::providers::TokenProvider;
use crate::types::TokenInfo;
use crate::cache::PostgresCache;
use crate::tcp_forwarder::TcpForwarder;
use tracing::{info, warn, debug, error};

pub async fn lookup_token(
    providers: &[Arc<dyn TokenProvider>], 
    address: &str,
    cache: Option<&PostgresCache>,
    forwarder: Option<&TcpForwarder>,
) -> TokenInfo {
    info!("Looking up token address: {}", address);
    
    let chain = detect_chain(address);
    info!("Detected chain: {}", chain);
    
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
            description: None,
            image_base64: None,
            image_url: None,
            website: None,
            twitter: None,
            telegram: None,
        };
    }
    
    // Check cache first if available
    if let Some(cache) = cache {
        if let Some(cached_info) = cache.get(address, chain).await {
            info!("Returning cached token info for {} on {}", address, chain);
            return cached_info;
        }
    }
    
    // Filter providers that support this address and belong to the detected chain
    let mut relevant_providers: Vec<&Arc<dyn TokenProvider>> = providers
        .iter()
        .filter(|p| p.supports_address(address) && p.chain() == chain)
        .collect();
    
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
            description: None,
            image_base64: None,
            image_url: None,
            website: None,
            twitter: None,
            telegram: None,
        };
    }
    
    let mut best_result: Option<TokenInfo> = None;
    
    for provider in relevant_providers {
        debug!("Trying provider {} (priority {})", provider.name(), provider.priority());
        match provider.get_token_info(address).await {
            Ok(info) => {
                if info.label.is_some() {
                    info!("Provider {} returned token WITH label: {:?}", provider.name(), info.label);
                    
                    // Cache the result
                    if let Some(cache) = cache {
                        if let Err(e) = cache.set(address, chain, &info).await {
                            warn!("Failed to cache token info: {}", e);
                        }
                    }
                    
                    // Forward to TCP endpoint if configured
                    if let Some(forwarder) = forwarder {
                        if chain == "solana" {
                            let _ = forwarder.send_token_address(address, chain).await;
                        }
                    }
                    
                    return info;
                } else {
                    info!("Provider {} returned token WITHOUT name, continuing search...", provider.name());
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
    
    if let Some(result) = best_result {
        warn!("No provider found a token name for {}, returning basic info", address);
        
        // Cache the basic info as well
        if let Some(cache) = cache {
            let _ = cache.set(address, chain, &result).await;
        }
        
        return result;
    }
    
    error!("All providers failed for address: {}", address);
    TokenInfo {
        chain: chain.to_string(),
        context: address.to_string(),
        label: None,
        long_label: None,
        decimals: 0,
        provider: "none".to_string(),
        description: None,
        image_base64: None,
        image_url: None,
        website: None,
        twitter: None,
        telegram: None,
    }
}

fn detect_chain(address: &str) -> &'static str {
    if address.starts_with("0x") && address.len() == 42 {
        "ethereum"
    } else if address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x") &&
              address.chars().all(|c| c.is_ascii_alphanumeric()) {
        "solana"
    } else {
        "unknown"
    }
}