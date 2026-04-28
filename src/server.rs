use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use crate::providers::Provider;
use crate::types::TokenInfo;
use tracing::{info, error, warn, debug};

pub async fn run_server(
    providers: Vec<Arc<dyn Provider + Send + Sync>>,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr).await?;
    info!("TCP listener bound to {}", addr);
    println!("Listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("New connection from: {}", addr);
                debug!("Connection details: {:?}", stream);
                
                let providers = providers.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, providers).await {
                        error!("Error handling connection from {}: {}", addr, e);
                    } else {
                        info!("Connection from {} closed successfully", addr);
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

async fn handle_connection(
    mut stream: TcpStream,
    providers: Vec<Arc<dyn Provider + Send + Sync>>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("Connection closed by client (EOF)");
                break;
            },
            Ok(_) => {
                let address = line.trim();
                if address.is_empty() {
                    debug!("Received empty line, skipping");
                    continue;
                }
                
                info!("Looking up token: {}", address);
                let token_info = lookup_token(&providers, address).await;
                let response = serde_json::to_string(&token_info)?;
                
                debug!("Response: {}", response);
                writer.write_all(response.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                
                info!("Sent response for token: {}", address);
            }
            Err(e) => {
                error!("Error reading line from connection: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn lookup_token(providers: &[Arc<dyn Provider + Send + Sync>], address: &str) -> TokenInfo {
    for provider in providers {
        if provider.supports_address(address) {
            debug!("Address {} supported by {}", address, provider.name());
            match provider.get_token_info(address).await {
                Ok(info) => {
                    info!("Provider {} returned token info for {}", provider.name(), address);
                    return info;
                },
                Err(e) => {
                    warn!("Provider {} failed for {}: {}", provider.name(), address, e);
                }
            }
        }
    }

    warn!("No provider could resolve token address: {}", address);
    TokenInfo {
        context: address.to_string(),
        label: "?".to_string(),
        decimals: 0,
    }
}