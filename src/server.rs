use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use crate::providers::TokenProvider;
use crate::lookup::lookup_token;
use tracing::{info, error, debug};

pub async fn run_server(
    providers: Vec<Arc<dyn TokenProvider>>,
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
    providers: Vec<Arc<dyn TokenProvider>>,
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