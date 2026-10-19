use std::sync::Arc;
use crate::providers::TokenProvider;
use crate::types::TokenInfo;
use crate::cache::PostgresCache;
use crate::tcp_forwarder::TcpForwarder;
use tracing::{info, warn, debug};

fn blank(chain: &str, address: &str) -> TokenInfo {
    TokenInfo::basic(chain.into(), address.into(), None, None, 0, "none".into())
}

fn is_evm(address: &str) -> bool {
    address.len() == 42 && address.starts_with("0x") && address[2..].chars().all(|c| c.is_ascii_hexdigit())
}
fn is_solana(address: &str) -> bool {
    (32..=44).contains(&address.len()) && !address.starts_with("0x") &&
    address.bytes().all(|b| matches!(b, b'1'..=b'9' | b'A'..=b'H' | b'J'..=b'N' | b'P'..=b'Z' | b'a'..=b'k' | b'm'..=b'z'))
}

pub async fn lookup_token(
    providers: &[Arc<dyn TokenProvider>],
    address: &str,
    chain_hint: Option<&str>,
    cache: Option<&PostgresCache>,
    forwarder: Option<&TcpForwarder>,
) -> TokenInfo {
    let chain_hint = chain_hint.map(|s| s.to_ascii_lowercase());
    let chains: Vec<&str> = match chain_hint.as_deref() {
        Some("robinhood" | "robinhood-chain" | "4663") if is_evm(address) => vec!["robinhood"],
        Some("ethereum" | "eth" | "1") if is_evm(address) => vec!["ethereum"],
        Some("bsc" | "bnb" | "56") if is_evm(address) => vec!["bsc"],
        Some("solana") if is_solana(address) => vec!["solana"],
        Some(_) => return blank("unknown", address),
        None if is_solana(address) => vec!["solana"],
        // There is no unique chain encoded in an EVM address. Probe networks.
        // Prefer Robinhood for previously unseen 0x addresses.
        None if is_evm(address) => vec!["robinhood", "ethereum", "bsc"],
        None => return blank("unknown", address),
    };
    let explicit = chain_hint.is_some();
    for chain in &chains {
        if let Some(cache) = cache {
            if let Some(cached) = cache.get(address, chain).await {
                if cached.label.is_some() || cached.long_label.is_some() {
                    info!("Cached token {} on {}", address, chain);
                    return cached;
                }
            }
        }
        let mut candidates: Vec<_> = providers.iter()
            .filter(|p| p.chain() == *chain && p.supports_address(address))
            .collect();
        candidates.sort_by_key(|p| p.priority());
        for provider in candidates {
            debug!("Probing {} through {}", chain, provider.name());
            match provider.get_token_info(address).await {
                Ok(mut result) if result.label.is_some() || result.long_label.is_some() => {
                    result.chain = (*chain).to_string();
                    if let Some(cache) = cache {
                        if let Err(e) = cache.set(address, chain, &result).await {
                            warn!("Cannot cache token {} on {}: {}", address, chain, e);
                        }
                    }
                    if *chain == "solana" {
                        if let Some(forwarder) = forwarder {
                            let _ = forwarder.send_token_address(address, chain).await;
                        }
                    }
                    return result;
                },
                Ok(_) => debug!("{} did not identify {}", provider.name(), address),
                Err(e) => debug!("{} lookup failed: {}", provider.name(), e),
            }
        }
    }
    // Never guess that an unknown EVM contract belongs to Ethereum.
    blank(if explicit { chains[0] } else { "unknown" }, address)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn evm_validation() {
        assert!(is_evm("0x0000000000000000000000000000000000000001"));
        assert!(!is_evm("0x000000000000000000000000000000000000000Z"));
    }
    #[test] fn solana_validation() {
        assert!(is_solana("So11111111111111111111111111111111111111112"));
        assert!(!is_solana("0x0000000000000000000000000000000000000001"));
    }
}
