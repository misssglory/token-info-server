//! Optional EVM source enrichment. Verified sources are cached separately from token metadata.
use crate::cache::PostgresCache;
use crate::source_fetch::{self, EtherscanContract};
use crate::source_scan::{build_source_analysis, SourceAnalysis};
use crate::types::TokenInfo;
use serde_json::{json, Value};
use tracing::{debug, warn};

fn evm_id(chain: &str) -> Option<&'static str> {
    match chain { "robinhood" => Some("4663"), "ethereum" => Some("1"), "bsc" => Some("56"), _ => None }
}

async fn fetch(chain: &str, address: &str) -> anyhow::Result<EtherscanContract> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .no_proxy().build()?;
    let key = std::env::var("ETHERSCAN_API_KEY").unwrap_or_default();
    if chain == "robinhood" {
        source_fetch::fetch_robinhood_source(&client, address, Some(&key)).await
    } else {
        anyhow::ensure!(!key.is_empty(), "ETHERSCAN_API_KEY is required for {chain} source lookups");
        source_fetch::fetch_source(&client, evm_id(chain).unwrap(), address, &key).await
    }
}

async fn compress(input: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    // zstd 6: materially stronger than LZ4, while remaining practical for interactive lookup.
    Ok(tokio::task::spawn_blocking(move || zstd::bulk::compress(&input, 6)).await??)
}
async fn decompress(input: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    Ok(tokio::task::spawn_blocking(move || zstd::stream::decode_all(input.as_slice())).await??)
}

/// Concurrent across independent TCP clients; no blocking I/O on the Tokio runtime.
/// Metadata is already resolved before this call; a cached source never triggers HTTP.
async fn get_source(chain: &str, address: &str, cache: Option<&PostgresCache>) -> anyhow::Result<Option<EtherscanContract>> {
    if evm_id(chain).is_none() { return Ok(None); }
    if let Some(cache) = cache {
        if let Some(bytes) = cache.get_sources(address, chain).await? {
            debug!(chain, address, "Verified contract source cache hit");
            return Ok(Some(serde_json::from_slice(&decompress(bytes).await?)?));
        }
    }
    let contract = fetch(chain, address).await?;
    if contract.source_files.is_empty() && contract.source_code.trim().is_empty() { return Ok(None); }
    if let Some(cache) = cache {
        let raw = serde_json::to_vec(&contract)?;
        let packed = compress(raw).await?;
        cache.set_sources(address, chain, packed).await?;
    }
    Ok(Some(contract))
}

pub async fn enrich(token: &TokenInfo, cache: Option<&PostgresCache>, raw: bool) -> Value {
    let mut out = serde_json::to_value(token).unwrap_or_else(|_| json!({}));
    if evm_id(&token.chain).is_none() || token.provider == "none" { return out; }
    match get_source(&token.chain, &token.context, cache).await {
        Ok(Some(contract)) => {
            let analysis: SourceAnalysis = build_source_analysis(&contract);
            out["launchpad"] = json!({
                "matches": analysis.launchpad_entities_found,
                "detected": analysis.launchpad_entity_found,
                "evidence_only": true
            });
            out["source_info"] = json!({
                "verified": analysis.verified,
                "provider": analysis.provider,
                "url": analysis.source_url,
                "contract_name": analysis.contract_name,
                "file_names": analysis.source_file_names,
                "compression": "zstd-6"
            });
            if raw { out["sources"] = serde_json::to_value(&contract.source_files).unwrap_or(json!([])); }
        }
        Ok(None) => { out["source_info"] = json!({"verified": false}); }
        Err(err) => {
            warn!(chain=%token.chain, address=%token.context, error=%format!("{err:#}"), "Source retrieval unavailable");
            out["source_info"] = json!({"verified": false, "status": "unavailable"});
        }
    }
    out
}
