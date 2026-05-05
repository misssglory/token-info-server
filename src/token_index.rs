use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;
use crate::types::{TokenSearchResult, TokenInfo};
use crate::tcp_forwarder::TcpForwarder;
use tracing::{info, warn, debug, error};
use sqlx::{PgPool, Row};

// Known token database - moved before TokenIndex struct
static KNOWN_TOKENS_DB: Lazy<HashMap<String, Vec<(String, String, u8)>>> = Lazy::new(|| {
    let mut db = HashMap::new();
    
    // Solana tokens
    db.insert("solana".to_string(), vec![
        ("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(), "USDC".to_string(), 6),
        ("So11111111111111111111111111111111111111112".to_string(), "SOL".to_string(), 9),
        ("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(), "USDT".to_string(), 6),
        ("mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So".to_string(), "mSOL".to_string(), 9),
    ]);
    
    // Ethereum tokens
    db.insert("ethereum".to_string(), vec![
        ("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(), "USDC".to_string(), 6),
        ("0xdac17f958d2ee523a2206206994597c13d831ec7".to_string(), "USDT".to_string(), 6),
        ("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599".to_string(), "WBTC".to_string(), 8),
        ("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(), "WETH".to_string(), 18),
    ]);
    
    // BSC tokens
    db.insert("bsc".to_string(), vec![
        ("0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d".to_string(), "USDC".to_string(), 18),
        ("0x55d398326f99059ff775485246999027b3197955".to_string(), "USDT".to_string(), 18),
        ("0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c".to_string(), "WBNB".to_string(), 18),
    ]);
    
    db
});

pub struct TokenIndex {
    cache: Option<Arc<crate::cache::PostgresCache>>,
    forwarder: Option<Arc<TcpForwarder>>,
}

impl TokenIndex {
    pub fn new(
        cache: Option<Arc<crate::cache::PostgresCache>>,
        forwarder: Option<Arc<TcpForwarder>>,
    ) -> Self {
        Self { cache, forwarder }
    }
    
    pub async fn search_by_name(
        &self,
        query: &str,
        chain: Option<&str>,
        limit: usize,
    ) -> Vec<TokenSearchResult> {
        info!("🔍 SEARCH REQUEST - Query: '{}'", query);
        info!("   Chain filter: {:?}, Limit: {}", chain, limit);
        
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        
        // Search in known tokens database
        let chains_to_search: Vec<String> = if let Some(chain) = chain {
            vec![chain.to_string()]
        } else {
            vec!["solana".to_string(), "ethereum".to_string(), "bsc".to_string()]
        };
        
        for chain_name in chains_to_search.iter() {
            if let Some(tokens) = KNOWN_TOKENS_DB.get(chain_name) {
                for (address, symbol, decimals) in tokens {
                    let symbol_lower = symbol.to_lowercase();
                    
                    let similarity = if symbol_lower == query_lower {
                        1.0
                    } else if symbol_lower.contains(&query_lower) || query_lower.contains(&symbol_lower) {
                        0.8
                    } else {
                        self.calculate_similarity(&symbol_lower, &query_lower)
                    };
                    
                    if similarity > 0.3 {
                        results.push(TokenSearchResult {
                            address: address.clone(),
                            chain: chain_name.clone(),
                            label: symbol.clone(),
                            long_label: None,
                            decimals: *decimals,
                            provider: "known-tokens".to_string(),
                            similarity: Some(similarity),
                        });
                    }
                }
            }
        }
        
        // Search in cached tokens from database
        if let Some(cache) = &self.cache {
            match self.search_cached_tokens(query, chain, limit).await {
                Ok(cached_results) => {
                    info!("Found {} cached results for '{}'", cached_results.len(), query);
                    results.extend(cached_results);
                }
                Err(e) => {
                    debug!("Failed to search cached tokens: {}", e);
                }
            }
        }
        
        // Sort by similarity and remove duplicates
        results.sort_by(|a, b| {
            let sim_a = a.similarity.unwrap_or(0.0);
            let sim_b = b.similarity.unwrap_or(0.0);
            sim_b.partial_cmp(&sim_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        let mut unique_results = Vec::new();
        let mut seen_addresses = std::collections::HashSet::new();
        for result in results {
            if !seen_addresses.contains(&result.address) {
                seen_addresses.insert(result.address.clone());
                unique_results.push(result);
            }
        }
        
        unique_results.truncate(limit);
        
        info!("📊 Search results count: {}", unique_results.len());
        for (i, result) in unique_results.iter().enumerate() {
            info!("   Result {}: {} ({}) - similarity: {:?}", 
                  i+1, result.label, result.address, result.similarity);
        }
        
        // If exactly one result found, send TG command
        if unique_results.len() == 1 {
            let token = &unique_results[0];
            info!("🎯 SINGLE RESULT FOUND for '{}' - Triggering TG command", query);
            info!("   Token: {} ({}) on chain: {}", token.label, token.address, token.chain);
            
            if let Some(forwarder) = &self.forwarder {
                info!("📡 Forwarder is configured");
                
                match forwarder.send_tg_command(
                    &token.address,
                    &token.label,
                    &token.chain,
                ).await {
                    Ok(_) => {
                        info!("✅ TG command sent successfully for token {}", token.label);
                    }
                    Err(e) => {
                        error!("❌ Failed to send TG command for {}: {}", token.address, e);
                    }
                }
            } else {
                warn!("⚠️ No forwarder configured! TG command not sent");
                warn!("   Check config.toml: tcp_endpoint.enabled = true");
                warn!("   And ensure TCP_ENDPOINT_HOST and TCP_ENDPOINT_PORT are set correctly");
            }
        } else if unique_results.is_empty() {
            info!("❌ No results found for query: {}", query);
        } else {
            info!("📚 Multiple results found ({} tokens) - NOT sending TG command", unique_results.len());
        }
        
        unique_results
    }
    
    async fn search_cached_tokens(
        &self,
        query: &str,
        chain: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TokenSearchResult>, sqlx::Error> {
        let cache = match &self.cache {
            Some(c) => c,
            None => return Ok(vec![]),
        };
        
        let pool = cache.pool();
        let query_lower = query.to_lowercase();
        
        let sql = r#"
            SELECT address, chain, token_info
            FROM token_cache
            WHERE token_info->>'label' ILIKE $1
               OR token_info->>'long_label' ILIKE $1
            LIMIT $2
        "#;
        
        let search_pattern = format!("%{}%", query_lower);
        
        let rows = sqlx::query_as::<_, (String, String, serde_json::Value)>(sql)
            .bind(&search_pattern)
            .bind(limit as i32)
            .fetch_all(pool)
            .await?;
        
        let mut results = Vec::new();
        for (address, chain_name, token_info_json) in rows {
            if let Ok(token_info) = serde_json::from_value::<TokenInfo>(token_info_json) {
                if let Some(label) = token_info.label {
                    let similarity = self.calculate_similarity(&label.to_lowercase(), &query_lower);
                    
                    results.push(TokenSearchResult {
                        address,
                        chain: chain_name,
                        label: label.clone(),
                        long_label: token_info.long_label,
                        decimals: token_info.decimals,
                        provider: token_info.provider,
                        similarity: Some(similarity),
                    });
                }
            }
        }
        
        Ok(results)
    }
    
    fn calculate_similarity(&self, s1: &str, s2: &str) -> f64 {
        if s1.is_empty() || s2.is_empty() {
            return 0.0;
        }
        
        let s1_lower = s1.to_lowercase();
        let s2_lower = s2.to_lowercase();
        
        // Levenshtein distance
        let s1_chars: Vec<char> = s1_lower.chars().collect();
        let s2_chars: Vec<char> = s2_lower.chars().collect();
        
        let mut matrix = vec![vec![0; s2_chars.len() + 1]; s1_chars.len() + 1];
        
        for i in 0..=s1_chars.len() {
            matrix[i][0] = i;
        }
        
        for j in 0..=s2_chars.len() {
            matrix[0][j] = j;
        }
        
        for i in 1..=s1_chars.len() {
            for j in 1..=s2_chars.len() {
                let cost = if s1_chars[i-1] == s2_chars[j-1] { 0 } else { 1 };
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i-1][j] + 1, matrix[i][j-1] + 1),
                    matrix[i-1][j-1] + cost
                );
            }
        }
        
        let distance = matrix[s1_chars.len()][s2_chars.len()] as f64;
        let max_len = std::cmp::max(s1_chars.len(), s2_chars.len()) as f64;
        
        1.0 - (distance / max_len)
    }
}