use crate::providers::TokenProvider;
use crate::types::{ProviderError, TokenInfo};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub struct EvmRpcProvider {
    chain: &'static str,
    rpc_url: String,
    client: reqwest::Client,
    priority: u8,
}

impl EvmRpcProvider {
    pub fn new(chain: &'static str, rpc_url: String, priority: u8) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(7))
            .build().expect("HTTP client");
        Self { chain, rpc_url, client, priority }
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value, ProviderError> {
        let response = self.client.post(&self.rpc_url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send().await.map_err(|e| ProviderError::HttpError(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ProviderError::HttpError(format!("RPC HTTP {}", response.status())));
        }
        let value: Value = response.json().await.map_err(|e| ProviderError::HttpError(e.to_string()))?;
        if let Some(error) = value.get("error") {
            return Err(ProviderError::HttpError(format!("RPC error: {}", error)));
        }
        value.get("result").cloned().ok_or_else(|| ProviderError::HttpError("RPC result missing".into()))
    }

    async fn call(&self, address: &str, selector: &str) -> Result<String, ProviderError> {
        self.rpc("eth_call", json!([{"to":address,"data":selector}, "latest"]))
            .await?.as_str().map(str::to_owned)
            .ok_or_else(|| ProviderError::HttpError("invalid eth_call result".into()))
    }

    fn decode_text(value: &str) -> Option<String> {
        let bytes = hex::decode(value.strip_prefix("0x")?).ok()?;
        // Standard dynamic ABI-encoded string: offset (32) + length (32) + data.
        if bytes.len() >= 64 {
            let offset = usize::try_from(u64::from_be_bytes(bytes.get(24..32)?.try_into().ok()?)).ok()?;
            if offset.checked_add(32)? <= bytes.len() {
                let len_at = offset.checked_add(24)?;
                let len = usize::try_from(u64::from_be_bytes(bytes.get(len_at..offset+32)?.try_into().ok()?)).ok()?;
                let from = offset + 32;
                let end = from.checked_add(len)?;
                if let Some(payload) = bytes.get(from..end) {
                    if let Ok(s) = std::str::from_utf8(payload) {
                        let s = s.trim_matches('\0').trim();
                        if !s.is_empty() { return Some(s.to_owned()); }
                    }
                }
            }
        }
        // Some tokens return a fixed bytes32 instead of string.
        if bytes.len() == 32 {
            let s = std::str::from_utf8(bytes.split(|b| *b == 0).next()?).ok()?.trim();
            if !s.is_empty() { return Some(s.to_owned()); }
        }
        None
    }

    pub async fn contract_exists(&self, address: &str) -> bool {
        match self.rpc("eth_getCode", json!([address,"latest"])).await {
            Ok(v) => v.as_str().map(|s| s != "0x" && s != "0x0" && !s.trim_start_matches("0x").chars().all(|c| c == '0')).unwrap_or(false),
            Err(e) => { tracing::debug!("{} getCode failed: {}", self.chain, e); false }
        }
    }
}

#[async_trait]
impl TokenProvider for EvmRpcProvider {
    fn name(&self) -> &str { "evm-rpc" }
    fn chain(&self) -> &str { self.chain }
    fn priority(&self) -> u8 { self.priority }
    fn supports_address(&self, address: &str) -> bool {
        address.len() == 42 && address.starts_with("0x") && address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        if !self.contract_exists(address).await { return Err(ProviderError::NotFound); }
        let symbol = match self.call(address,"0x95d89b41").await {
            Ok(v) => Self::decode_text(&v), Err(_) => None
        };
        let name = match self.call(address,"0x06fdde03").await {
            Ok(v) => Self::decode_text(&v), Err(_) => None
        };
        // A random deployed contract is not necessarily an ERC-20 token.
        if symbol.is_none() && name.is_none() { return Err(ProviderError::NotFound); }
        let decimals = self.call(address,"0x313ce567").await.ok()
            .and_then(|s| u128::from_str_radix(s.trim_start_matches("0x"),16).ok())
            .and_then(|n| u8::try_from(n).ok()).unwrap_or(18);
        Ok(TokenInfo::basic(self.chain.to_string(), address.to_string(), symbol, name, decimals, self.name().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decode_dynamic_symbol() {
        let mut encoded = vec![0u8; 96];
        encoded[31] = 32;
        encoded[63] = 4;
        encoded[64..68].copy_from_slice(b"TEST");
        assert_eq!(EvmRpcProvider::decode_text(&format!("0x{}", hex::encode(encoded))), Some("TEST".into()));
    }
    #[test]
    fn decode_bytes32_symbol() {
        let mut encoded = [0u8;32];
        encoded[..4].copy_from_slice(b"TEST");
        assert_eq!(EvmRpcProvider::decode_text(&format!("0x{}", hex::encode(encoded))), Some("TEST".into()));
    }
}
