use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, BufReader, AsyncBufReadExt};
use serde_json::json;
use tracing::{info, error, debug, warn};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct TcpForwarderConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub forward_tokens: bool,
}

impl TcpForwarderConfig {
    pub fn from_config(config: &crate::config::TcpEndpointConfig) -> Self {
        Self {
            enabled: config.enabled,
            host: config.host.clone(),
            port: config.port,
            forward_tokens: config.forward_tokens,
        }
    }
}

pub struct TcpForwarder {
    config: TcpForwarderConfig,
    connection: Arc<Mutex<Option<TcpStream>>>,
}

impl TcpForwarder {
    pub fn new(config: TcpForwarderConfig) -> Self {
        Self {
            config,
            connection: Arc::new(Mutex::new(None)),
        }
    }
    
    pub async fn connect(&self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        let addr = format!("{}:{}", self.config.host, self.config.port);
        info!("Connecting to TCP endpoint at {}", addr);
        
        match TcpStream::connect(&addr).await {
            Ok(stream) => {
                let mut conn = self.connection.lock().await;
                *conn = Some(stream);
                info!("Connected to TCP endpoint at {}", addr);
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to TCP endpoint {}: {}", addr, e);
                Err(e.into())
            }
        }
    }
    
    pub async fn send_token_address(&self, address: &str, chain: &str) -> Result<()> {
        if !self.config.enabled || !self.config.forward_tokens {
            return Ok(());
        }
        
        let request = json!({
            "type": "query",
            "command": "get_executor",
            "args": {
                "mint": address
            }
        });
        
        self.send_json(&request).await?;
        debug!("Forwarded token address {} (chain: {}) to TCP endpoint", address, chain);
        Ok(())
    }
    
    pub async fn send_bulk_addresses(&self, addresses: &[(String, String)]) -> Result<()> {
        if !self.config.enabled || !self.config.forward_tokens {
            return Ok(());
        }
        
        for (address, chain) in addresses {
            if let Err(e) = self.send_token_address(address, chain).await {
                warn!("Failed to forward address {}: {}", address, e);
            }
        }
        
        Ok(())
    }
    
    async fn send_json(&self, data: &serde_json::Value) -> Result<()> {
        let mut conn_guard = self.connection.lock().await;
        
        if conn_guard.is_none() {
            // Try to reconnect
            drop(conn_guard);
            self.connect().await?;
            conn_guard = self.connection.lock().await;
        }
        
        if let Some(conn) = conn_guard.as_mut() {
            let json_str = serde_json::to_string(data)? + "\n";
            conn.write_all(json_str.as_bytes()).await?;
            conn.flush().await?;
            
            // Read response (optional)
            let mut reader = BufReader::new(conn);
            let mut response = String::new();
            match reader.read_line(&mut response).await {
                Ok(_) => {
                    debug!("Received response from TCP endpoint: {}", response.trim());
                }
                Err(e) => {
                    warn!("Failed to read response from TCP endpoint: {}", e);
                }
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("No TCP connection available"))
        }
    }
    
    pub async fn health_check(&self) -> bool {
        if !self.config.enabled {
            return true;
        }
        
        let request = json!({
            "type": "query",
            "command": "get_health",
            "args": {}
        });
        
        self.send_json(&request).await.is_ok()
    }
}