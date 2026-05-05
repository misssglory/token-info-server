use crate::types::TgCommand;
use anyhow::Result;
use serde_json::json;
use std::io;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

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
        info!("🔌 Connecting to TCP endpoint at {}", addr);

        match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(stream)) => {
                let peer_addr = stream.peer_addr()?;
                let local_addr = stream.local_addr()?;
                let mut conn = self.connection.lock().await;
                *conn = Some(stream);
                info!("✅ Connected to {} (local: {})", peer_addr, local_addr);
                Ok(())
            }
            Ok(Err(e)) => {
                error!("❌ Failed to connect to TCP endpoint {}: {}", addr, e);
                Err(e.into())
            }
            Err(_) => {
                error!("❌ Connection timeout to TCP endpoint {}", addr);
                Err(io::Error::new(io::ErrorKind::TimedOut, "Connection timeout").into())
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
        debug!(
            "Forwarded token address {} (chain: {}) to TCP endpoint",
            address, chain
        );
        Ok(())
    }

    pub async fn send_tg_command(&self, mint: &str, symbol: &str, chain: &str) -> Result<()> {
        if !self.config.enabled {
            info!("📡 TCP forwarder disabled, skipping TG command");
            return Ok(());
        }

        info!("═══════════════════════════════════════════════════════════");
        info!("🎯 SENDING TG COMMAND - BEAVER SEARCH");
        info!("═══════════════════════════════════════════════════════════");
        info!(
            "📤 Target address: {}:{}",
            self.config.host, self.config.port
        );
        info!("🪙 Token: {} ({})", symbol, mint);
        info!("⛓️ Chain: {}", chain);

        // Generate timestamp in microseconds
        let now = SystemTime::now();
        let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default();
        let timestamp_micros =
            since_epoch.as_secs() * 1_000_000 + since_epoch.subsec_micros() as u64;

        // Channel name
        let channel = "Token name search";

        // Create the exact format
        let tg_message = json!({
            "context": mint,
            "command": "tg",
            "args": {
                "channel": channel,
                "ts": timestamp_micros,
                "text": mint
            }
        });

        let json_string = serde_json::to_string_pretty(&tg_message)?;
        info!("📦 Payload to send:");
        info!("{}", json_string);
        info!("───────────────────────────────────────────────────────────");

        match self.send_json(&tg_message).await {
            Ok(_) => {
                info!(
                    "✅ TG command sent successfully to {}:{}",
                    self.config.host, self.config.port
                );
                info!("═══════════════════════════════════════════════════════════");
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to send TG command: {}", e);
                info!("Attempting to reconnect and retry...");

                // Clear broken connection
                let mut conn_guard = self.connection.lock().await;
                *conn_guard = None;
                drop(conn_guard);

                // Try to reconnect
                if self.connect().await.is_ok() {
                    info!("Reconnected, retrying send...");
                    match self.send_json(&tg_message).await {
                        Ok(_) => {
                            info!("✅ TG command sent successfully after reconnect");
                            info!("═══════════════════════════════════════════════════════════");
                            Ok(())
                        }
                        Err(e2) => {
                            error!("❌ Failed even after reconnect: {}", e2);
                            info!("═══════════════════════════════════════════════════════════");
                            Err(e2)
                        }
                    }
                } else {
                    error!("❌ Reconnection failed");
                    info!("═══════════════════════════════════════════════════════════");
                    Err(e)
                }
            }
        }
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
            drop(conn_guard);
            info!("No connection, establishing new connection...");
            self.connect().await?;
            conn_guard = self.connection.lock().await;
        }

        if let Some(conn) = conn_guard.as_mut() {
            let json_str = serde_json::to_string(data)? + "\n";
            info!("📝 Raw data being sent: {}", json_str.trim());

            // Write with timeout
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                conn.write_all(json_str.as_bytes()),
            )
            .await
            {
                Ok(Ok(_)) => {
                    conn.flush().await?;
                    info!("💾 Data written to socket successfully");
                }
                Ok(Err(e)) => {
                    error!("💥 Write error: {}", e);
                    *conn_guard = None;
                    return Err(e.into());
                }
                Err(_) => {
                    error!("⏰ Write timeout");
                    *conn_guard = None;
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "Write timeout").into());
                }
            }

            // Try to read response
            let mut reader = BufReader::new(conn);
            let mut response = String::new();
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(500),
                reader.read_line(&mut response),
            )
            .await
            {
                Ok(Ok(0)) => {
                    info!("🔌 Connection closed by endpoint");
                    *conn_guard = None;
                }
                Ok(Ok(_)) => {
                    info!("📨 Received response: {}", response.trim());
                }
                Ok(Err(e)) => {
                    debug!("Failed to read response (non-critical): {}", e);
                }
                Err(_) => {
                    debug!("⏰ No response received (timeout)");
                }
            }

            Ok(())
        } else {
            error!("❌ No TCP connection available");
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

        match self.send_json(&request).await {
            Ok(_) => true,
            Err(e) => {
                debug!("Health check failed: {}", e);
                false
            }
        }
    }
}

