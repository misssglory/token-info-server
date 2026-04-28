use async_trait::async_trait;
use crate::types::{TokenInfo, ProviderError};
use crate::providers::TokenProvider;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub struct MetadataApiProvider {
    rpc_client: RpcClient,
    priority: u8,
}

impl MetadataApiProvider {
    pub fn new(rpc_url: String, priority: u8) -> Self {
        tracing::info!("Initializing Metadata API provider with priority {}", priority);
        Self {
            rpc_client: RpcClient::new(rpc_url),
            priority,
        }
    }
    
    async fn fetch_metaplex_metadata(&self, mint_address: &str) -> Option<(String, Option<String>, u8)> {
        // Metaplex Token Metadata Program ID
        let metadata_program_id = Pubkey::from_str("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s").ok()?;
        let mint_pubkey = Pubkey::from_str(mint_address).ok()?;
        
        // Derive the metadata account PDA
        let seeds = &[
            b"metadata",
            metadata_program_id.as_ref(),
            mint_pubkey.as_ref(),
        ];
        
        let (metadata_pubkey, _) = Pubkey::find_program_address(seeds, &metadata_program_id);
        tracing::debug!("Looking for metadata at: {}", metadata_pubkey);
        
        // Fetch the metadata account
        if let Ok(account) = self.rpc_client.get_account(&metadata_pubkey) {
            let data = &account.data;
            let mut offset = 1; // Skip key byte
            
            // Skip update_authority (32 bytes)
            if offset + 32 > data.len() { return None; }
            offset += 32;
            
            // Skip mint (32 bytes)
            if offset + 32 > data.len() { return None; }
            offset += 32;
            
            // Read name
            if offset + 4 > data.len() { return None; }
            let name_len = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
            offset += 4;
            
            if offset + name_len > data.len() { return None; }
            let name = String::from_utf8_lossy(&data[offset..offset+name_len]).to_string();
            offset += name_len;
            
            // Read symbol
            if offset + 4 > data.len() { return None; }
            let symbol_len = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
            offset += 4;
            
            if offset + symbol_len > data.len() { return None; }
            let symbol = String::from_utf8_lossy(&data[offset..offset+symbol_len]).to_string();
            
            // Clean up null bytes and empty strings
            let name = name.trim_matches(char::from(0)).to_string();
            let symbol = symbol.trim_matches(char::from(0)).to_string();
            
            if name.is_empty() {
                return None;
            }
            
            // Parse label (short name) and long_label (full name)
            let (label, long_label) = if !symbol.is_empty() && symbol != name {
                // Symbol is the short name (e.g., "Bubbles")
                // Name is the long description (e.g., "Michael jackson Chimpanzee")
                (symbol.clone(), Some(name.clone()))
            } else {
                // If no symbol, use name as label and no long_label
                (name.clone(), None)
            };
            
            tracing::info!("Found Metaplex metadata - label: {}, long_label: {:?}", label, long_label);
            
            // Get decimals from mint account
            let decimals = self.get_token_decimals(mint_address).await.unwrap_or(6);
            return Some((label, long_label, decimals));
        }
        
        None
    }
    
    async fn get_token_decimals(&self, mint_address: &str) -> Option<u8> {
        let mint_pubkey = Pubkey::from_str(mint_address).ok()?;
        if let Ok(account) = self.rpc_client.get_account(&mint_pubkey) {
            // Mint account layout: 
            // mint_authority (32 bytes), supply (8 bytes), decimals (1 byte)
            if account.data.len() >= 45 {
                return Some(account.data[44]); // decimals is at offset 44
            }
        }
        None
    }
}

#[async_trait]
impl TokenProvider for MetadataApiProvider {
    fn name(&self) -> &str {
        "metadata-api"
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
        tracing::info!("Fetching token metadata from Metaplex for: {}", address);
        
        if let Some((label, long_label, decimals)) = self.fetch_metaplex_metadata(address).await {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(label),
                long_label,
                decimals,
                provider: self.name().to_string(),
            });
        }
        
        tracing::debug!("No Metaplex metadata found for: {}", address);
        Err(ProviderError::NotFound)
    }
}