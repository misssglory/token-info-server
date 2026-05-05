use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use std::time::Instant;
use crate::providers::TokenProvider;
use crate::lookup::lookup_token;
use crate::cache::PostgresCache;
use crate::tcp_forwarder::TcpForwarder;
use tracing::{info, error, debug, warn, trace};
use serde_json::Value;

pub async fn run_server(
    providers: Vec<Arc<dyn TokenProvider>>,
    cache: Option<Arc<PostgresCache>>,
    forwarder: Option<Arc<TcpForwarder>>,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;
    info!("TCP server listening on {}", addr);
    println!("✅ Token Info Server running on {}", addr);
    println!("📡 Accepting TCP connections...");
    println!("💡 Accepts plain addresses or JSON: {{\"context\":\"address\"}}");
    
    if cache.is_some() {
        println!("💾 PostgreSQL cache enabled");
    }
    
    if let Some(f) = &forwarder {
        if f.health_check().await {
            println!("🔗 TCP forwarder connected to endpoint");
        } else {
            println!("⚠️  TCP forwarder not connected");
        }
    }

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
                
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, providers, cache, forwarder, conn_id, client_addr).await {
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

fn extract_address(input: &str) -> String {
    if let Ok(json) = serde_json::from_str::<Value>(input) {
        if let Some(context) = json.get("context").and_then(|v| v.as_str()) {
            trace!("Extracted address from JSON context field: {}", context);
            return context.to_string();
        }
        if let Some(address) = json.get("address").and_then(|v| v.as_str()) {
            trace!("Extracted address from JSON address field: {}", address);
            return address.to_string();
        }
        if let Some(mint) = json.get("mint").and_then(|v| v.as_str()) {
            trace!("Extracted address from JSON mint field: {}", mint);
            return mint.to_string();
        }
        if let Some(s) = json.as_str() {
            trace!("Extracted address from JSON string: {}", s);
            return s.to_string();
        }
        trace!("JSON detected but no address field found, using raw input");
    }
    
    input.trim().to_string()
}

async fn handle_connection(
    mut stream: TcpStream,
    providers: Vec<Arc<dyn TokenProvider>>,
    cache: Option<Arc<PostgresCache>>,
    forwarder: Option<Arc<TcpForwarder>>,
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
                
                let address = extract_address(raw_input);
                
                info!("[conn:{}] 📥 Request #{}: input='{}' -> address='{}'", 
                      conn_id, request_count, raw_input, address);
                
                let token_info = lookup_token(
                    &providers, 
                    &address, 
                    cache.as_ref().map(|c| c.as_ref()),
                    forwarder.as_ref().map(|f| f.as_ref()),
                ).await;
                
                let response = serde_json::to_string(&token_info)?;
                let response_size = response.len();
                
                if let Some(label) = &token_info.label {
                    info!("[conn:{}] ✓ Request #{}: Found '{}' (provider: {})", 
                          conn_id, request_count, label, token_info.provider);
                } else {
                    warn!("[conn:{}] ⚠ Request #{}: No label found for '{}' (provider: {})", 
                          conn_id, request_count, address, token_info.provider);
                }
                
                writer.write_all(response.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                
                let request_duration = request_start.elapsed();
                debug!("[conn:{}] Request #{} completed in {:?}, response size: {} bytes", 
                       conn_id, request_count, request_duration, response_size);
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