use async_trait::async_trait;
use crate::types::{TokenInfo, ProviderError};

#[async_trait]
pub trait TokenProvider: Send + Sync {
    fn name(&self) -> &str;
    fn chain(&self) -> &str;
    fn priority(&self) -> u8;
    fn supports_address(&self, address: &str) -> bool;
    async fn get_token_info(&self, address: &str) -> Result<TokenInfo, ProviderError>;
}