use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use crate::types::{TokenInfo, ProviderError};
use crate::providers::TokenProvider;

pub struct PumpfunProvider {
    rpc_client: RpcClient,
    priority: u8,
}

impl PumpfunProvider {
    pub fn new(rpc_url: String, priority: u8) -> Self {
        tracing::info!("Initializing Pump.fun provider with priority {}", priority);
        Self {
            rpc_client: RpcClient::new(rpc_url),
            priority,
        }
    }
    
    async fn get_pumpfun_metadata(&self, mint_address: &str) -> Option<(String, u8)> {
        let pump_program_id = Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").ok()?;
        let mint_pubkey = Pubkey::from_str(mint_address).ok()?;
        
        let seeds = &[b"metadata", mint_pubkey.as_ref()];
        let (metadata_pubkey, _) = Pubkey::find_program_address(seeds, &pump_program_id);
        
        if let Ok(account) = self.rpc_client.get_account(&metadata_pubkey) {
            let mut offset = 0;
            
            if offset + 4 > account.data.len() { return None; }
            let name_len = u32::from_le_bytes([
                account.data[offset], account.data[offset+1], 
                account.data[offset+2], account.data[offset+3]
            ]) as usize;
            offset += 4;
            
            if offset + name_len > account.data.len() { return None; }
            let name = String::from_utf8(account.data[offset..offset+name_len].to_vec()).ok()?;
            offset += name_len;
            
            if offset + 4 > account.data.len() { return None; }
            let symbol_len = u32::from_le_bytes([
                account.data[offset], account.data[offset+1], 
                account.data[offset+2], account.data[offset+3]
            ]) as usize;
            offset += 4;
            
            if offset + symbol_len > account.data.len() { return None; }
            let symbol = String::from_utf8(account.data[offset..offset+symbol_len].to_vec()).ok()?;
            
            return Some((format!("{} ({})", name, symbol), 6));
        }
        
        None
    }
}

#[async_trait]
impl TokenProvider for PumpfunProvider {
    fn name(&self) -> &str {
        "pumpfun"
    }
    
    fn chain(&self) -> &str {
        "solana"
    }
    
    fn priority(&self) -> u8 {
        self.priority
    }
    
    fn supports_address(&self, address: &str) -> bool {
        address.len() >= 32 && address.len() <= 44 && !address.starts_with("0x")
    }
    
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError> {
        if let Some((name, decimals)) = self.get_pumpfun_metadata(address).await {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(name),
                decimals,
                provider: self.name().to_string(),
            });
        }
        
        Err(ProviderError::NotFound)
    }
}