use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use std::time::Instant;
use crate::providers::TokenProvider;
use crate::lookup::lookup_token;
use crate::cache::PostgresCache;
use crate::tcp_forwarder::TcpForwarder;
use crate::token_index::TokenIndex;
use crate::types::{TokenSearchRequest, TokenSearchResult};
use tracing::{info, error, debug, warn, trace};
use serde_json::{Value, json};

pub async fn run_server(
    providers: Vec<Arc<dyn TokenProvider>>,
    cache: Option<Arc<PostgresCache>>,
    forwarder: Option<Arc<TcpForwarder>>,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;
    
    // Create token index with forwarder for TG commands
    let token_index = Arc::new(TokenIndex::new(cache.clone(), forwarder.clone()));
    
    info!("TCP server listening on {}", addr);
    info!("✅ Token Info Server running on {}", addr);
    info!("📡 Accepting TCP connections...");
    info!("💡 Accepts:");
    info!("   - Plain addresses: 'So111...'");
    info!("   - JSON lookup: {{\"context\":\"address\"}}");
    info!("   - JSON search: {{\"type\":\"search\",\"query\":\"BEAVER\",\"limit\":5}}");

    let mut connection_id = 0;
    
    loop {
        match listener.accept().await {
            Ok((stream, client_addr)) => {
                connection_id += 1;
                let conn_id = connection_id;
                info!("[conn:{}] New TCP connection from: {}", conn_id, client_addr);
                
                let providers = providers.clone();
                let cache = cache.clone();
                let forwarder = forwarder.clone();
                let token_index = token_index.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, providers, cache, forwarder, token_index, conn_id, client_addr).await {
                        error!("[conn:{}] Error handling connection: {}", conn_id, e);
                    } else {
                        info!("[conn:{}] Connection closed successfully", conn_id);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                break;
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
enum RequestType {
    Lookup(String, Option<String>),
    Search(TokenSearchRequest),
    Sources(String, Option<String>),
}

fn parse_request(input: &str) -> Result<RequestType, String> {
    let trimmed = input.trim();
    
    if trimmed.is_empty() {
        return Err("Empty request".to_string());
    }
    
    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
        if json.get("type").and_then(|v| v.as_str()) == Some("sources") {
            let address = json.get("context").or_else(|| json.get("address"))
                .and_then(|v| v.as_str()).ok_or("Missing address for sources")?;
            let chain = json.get("chain").and_then(|v| v.as_str()).map(str::to_owned);
            return Ok(RequestType::Sources(address.to_owned(), chain));
        }
        // Check if it's a search request
        let is_search = json.get("type")
            .and_then(|v| v.as_str())
            .map(|t| t == "search")
            .unwrap_or(false);
        
        let has_query = json.get("query").is_some();
        let has_address_field = json.get("context").is_some() || 
                                json.get("address").is_some() || 
                                json.get("mint").is_some();
        
        if is_search || (has_query && !has_address_field) {
            let query = json.get("query")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'query' field in search request")?
                .to_string();
            
            let chain = json.get("chain")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            let limit = json.get("limit")
                .and_then(|v| v.as_u64())
                .map(|l| l as usize);
            
            return Ok(RequestType::Search(TokenSearchRequest {
                query,
                chain,
                limit,
            }));
        }
        
        // Optional chain override for EVM address ambiguity.
        let chain = json.get("chain").and_then(|v| v.as_str()).map(str::to_owned);
        // Extract address from JSON
        if let Some(context) = json.get("context").and_then(|v| v.as_str()) {
            return Ok(RequestType::Lookup(context.to_string(), chain));
        }
        if let Some(address) = json.get("address").and_then(|v| v.as_str()) {
            return Ok(RequestType::Lookup(address.to_string(), chain));
        }
        if let Some(mint) = json.get("mint").and_then(|v| v.as_str()) {
            return Ok(RequestType::Lookup(mint.to_string(), chain));
        }
        if let Some(s) = json.as_str() {
            return Ok(RequestType::Lookup(s.to_string(), chain));
        }
    }
    
    // Treat as plain address
    Ok(RequestType::Lookup(trimmed.to_string(), None))
}

async fn handle_connection(
    mut stream: TcpStream,
    providers: Vec<Arc<dyn TokenProvider>>,
    cache: Option<Arc<PostgresCache>>,
    forwarder: Option<Arc<TcpForwarder>>,
    token_index: Arc<TokenIndex>,
    conn_id: u32,
    client_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut request_count = 0;
    let connection_start = Instant::now();

    info!("[conn:{}] Starting to process requests from {}", conn_id, client_addr);

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                trace!("[conn:{}] Client closed connection (EOF)", conn_id);
                break;
            },
            Ok(_) => {
                let raw_input = line.trim();
                if raw_input.is_empty() {
                    debug!("[conn:{}] Received empty line, skipping", conn_id);
                    continue;
                }
                
                request_count += 1;
                let request_start = Instant::now();
                
                match parse_request(raw_input) {
                    Ok(RequestType::Lookup(address, chain)) => {
                        info!("[conn:{}] 📥 Lookup request #{}: address='{}'", 
                              conn_id, request_count, address);
                        
                        let token_info = lookup_token(
                            &providers, 
                            &address,
                            chain.as_deref(),
                            cache.as_ref().map(|c| c.as_ref()),
                            forwarder.as_ref().map(|f| f.as_ref()),
                        ).await;

                        // A DB failure must not be disguised as a cache miss and
                        // must not produce an accidental provider request.
                        let token_info = match token_info {
                            Ok(info) => info,
                            Err(e) => {
                                let response = json!({
                                    "success": false,
                                    "error": format!("cache lookup failed: {e:#}")
                                });
                                writer.write_all(serde_json::to_string(&response)?.as_bytes()).await?;
                                writer.write_all(b"\n").await?;
                                writer.flush().await?;
                                continue;
                            }
                        };
                        let enriched = crate::source_service::enrich(&token_info, cache.as_deref(), false).await;
                        let response = serde_json::to_string(&enriched)?;
                        
                        if let Some(label) = &token_info.label {
                            info!("[conn:{}] ✓ Found: '{}'", conn_id, label);
                        } else {
                            warn!("[conn:{}] ⚠ No label found for '{}'", conn_id, address);
                        }
                        
                        writer.write_all(response.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Ok(RequestType::Sources(address, chain)) => {
                        let info = lookup_token(&providers, &address, chain.as_deref(),
                            cache.as_deref(), forwarder.as_deref()).await;
                        let response = match info {
                            Ok(token) => crate::source_service::enrich(&token, cache.as_deref(), true).await,
                            Err(e) => json!({"error": format!("lookup failed: {e:#}")}),
                        };
                        writer.write_all(serde_json::to_string(&response)?.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Ok(RequestType::Search(search_req)) => {
                        info!("[conn:{}] 🔍 Search request #{}: query='{}', chain='{:?}', limit={:?}", 
                              conn_id, request_count, search_req.query, search_req.chain, search_req.limit);
                        
                        let limit = search_req.limit.unwrap_or(10);
                        let results = token_index
                            .search_by_name(&search_req.query, search_req.chain.as_deref(), limit)
                            .await;
                        
                        let response = json!({
                            "success": true,
                            "query": search_req.query,
                            "chain": search_req.chain,
                            "count": results.len(),
                            "results": results,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        });
                        
                        info!("[conn:{}] Found {} results for '{}'", conn_id, results.len(), search_req.query);
                        
                        writer.write_all(serde_json::to_string(&response)?.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    Err(e) => {
                        error!("[conn:{}] Failed to parse request: {}", conn_id, e);
                        let error_response = json!({
                            "success": false,
                            "error": format!("Invalid request: {}", e),
                            "raw_input": raw_input,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        });
                        writer.write_all(serde_json::to_string(&error_response)?.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                }
                
                writer.flush().await?;
                
                let request_duration = request_start.elapsed();
                debug!("[conn:{}] Request #{} completed in {:?}", 
                       conn_id, request_count, request_duration);
            }
            Err(e) => {
                error!("[conn:{}] Error reading from connection: {}", conn_id, e);
                break;
            }
        }
    }
    
    let connection_duration = connection_start.elapsed();
    info!("[conn:{}] Connection summary: {} requests processed in {:?}", 
          conn_id, request_count, connection_duration);
    
    Ok(())
}