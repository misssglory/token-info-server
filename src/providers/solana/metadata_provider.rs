use crate::providers::TokenProvider;
use crate::types::{ProviderError, TokenInfo};
use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::metadata_pointer::MetadataPointer;
use spl_token_2022::extension::BaseStateWithExtensions;
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::Mint as Token2022Mint;
use spl_token_metadata_interface::state::TokenMetadata;
use std::str::FromStr;

pub struct MetadataApiProvider {
    rpc_client: RpcClient,
    priority: u8,
}

impl MetadataApiProvider {
    pub fn new(rpc_url: String, priority: u8) -> Self {
        tracing::info!(
            "Initializing Metadata API provider with priority {}",
            priority
        );
        Self {
            rpc_client: RpcClient::new(rpc_url),
            priority,
        }
    }
    
    async fn fetch_metaplex_metadata(
        &self,
        mint_address: &str,
    ) -> Option<(String, Option<String>, u8)> {
        let metadata_program_id =
            Pubkey::from_str("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s").ok()?;
        let mint_pubkey = Pubkey::from_str(mint_address).ok()?;
        let seeds = &[
            b"metadata",
            metadata_program_id.as_ref(),
            mint_pubkey.as_ref(),
        ];
        let (metadata_pubkey, _) = Pubkey::find_program_address(seeds, &metadata_program_id);
        tracing::debug!("Looking for metadata at: {}", metadata_pubkey);
        
        if let Ok(account) = self.rpc_client.get_account(&metadata_pubkey) {
            let data = &account.data;
            let mut offset = 1; // Skip the discriminator byte
            
            // Skip key (32 bytes)
            if offset + 32 > data.len() {
                return None;
            }
            offset += 32;
            
            // Skip update authority (32 bytes)
            if offset + 32 > data.len() {
                return None;
            }
            offset += 32;
            
            // Read name length
            if offset + 4 > data.len() {
                return None;
            }
            let name_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;
            
            // Read name
            if offset + name_len > data.len() {
                return None;
            }
            let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
            offset += name_len;
            
            // Read symbol length
            if offset + 4 > data.len() {
                return None;
            }
            let symbol_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;
            
            // Read symbol
            if offset + symbol_len > data.len() {
                return None;
            }
            let symbol = String::from_utf8_lossy(&data[offset..offset + symbol_len]).to_string();
            
            let name = name.trim_matches(char::from(0)).to_string();
            let symbol = symbol.trim_matches(char::from(0)).to_string();
            
            if name.is_empty() {
                return None;
            }
            
            let (label, long_label) = if !symbol.is_empty() && symbol != name {
                (symbol.clone(), Some(name.clone()))
            } else {
                (name.clone(), None)
            };
            
            tracing::info!(
                "Found Metaplex metadata - label: {}, long_label: {:?}",
                label,
                long_label
            );
            
            let decimals = self.get_token_decimals(mint_address).await.unwrap_or(6);
            return Some((label, long_label, decimals));
        }
        None
    }
    
    async fn get_token_decimals(&self, mint_address: &str) -> Option<u8> {
        let mint_pubkey = Pubkey::from_str(mint_address).ok()?;
        if let Ok(account) = self.rpc_client.get_account(&mint_pubkey) {
            if account.data.len() >= 45 {
                return Some(account.data[44]);
            }
        }
        None
    }
    
    async fn fetch_token_2022_metadata(
        &self,
        mint_address: &str,
    ) -> Option<(String, Option<String>, u8)> {
        let mint_pubkey = Pubkey::from_str(mint_address).ok()?;
        let account = self.rpc_client.get_account(&mint_pubkey).ok()?;
        
        if account.owner != spl_token_2022::id() {
            return None;
        }
        
        let mint_state = StateWithExtensions::<Token2022Mint>::unpack(&account.data).ok()?;
        let decimals = mint_state.base.decimals;
        let _metadata_pointer = mint_state.get_extension::<MetadataPointer>().ok();
        let token_metadata = mint_state
            .get_variable_len_extension::<TokenMetadata>()
            .ok()?;
        
        let name = token_metadata
            .name
            .trim_matches(char::from(0))
            .trim()
            .to_string();
        let symbol = token_metadata
            .symbol
            .trim_matches(char::from(0))
            .trim()
            .to_string();
        
        if name.is_empty() && symbol.is_empty() {
            return None;
        }
        
        let (label, long_label) = if !symbol.is_empty() && symbol != name {
            (symbol, if name.is_empty() { None } else { Some(name) })
        } else {
            (name, None)
        };
        
        Some((label, long_label, decimals))
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
        tracing::info!("Fetching token metadata for: {}", address);
        
        if let Some((label, long_label, decimals)) = self.fetch_metaplex_metadata(address).await {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(label),
                long_label,
                decimals,
                provider: self.name().to_string(),
                description: None,
                image_base64: None,
                image_url: None,
                website: None,
                twitter: None,
                telegram: None,
            });
        }
        
        if let Some((label, long_label, decimals)) = self.fetch_token_2022_metadata(address).await {
            return Ok(TokenInfo {
                chain: self.chain().to_string(),
                context: address.to_string(),
                label: Some(label),
                long_label,
                decimals,
                provider: "token-2022-metadata".to_string(),
                description: None,
                image_base64: None,
                image_url: None,
                website: None,
                twitter: None,
                telegram: None,
            });
        }
        
        tracing::debug!("No Metaplex or Token-2022 metadata found for: {}", address);
        Err(ProviderError::NotFound)
    }
}