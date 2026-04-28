mod providers;
mod server;
mod types;

use clap::Parser;
use dotenv::dotenv;
use providers::{Provider, SolanaProvider, EthereumProvider, BscProvider};
use types::TokenInfo;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Token address to lookup (optional, if not provided starts TCP server)
    token_address: Option<String>,

    /// TCP port to listen on (default: 8080)
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Host to bind to (default: 127.0.0.1)
    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,
    
    /// Enable verbose logging
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Initialize tracing subscriber
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .init();
    
    let cli = Cli::parse();
    
    // Set log level based on verbose flag
    if cli.verbose {
        // Use set_global_default instead of with_default for simplicity
        tracing::info!("Verbose logging enabled");
        // Note: The logging level is already set above; for dynamic changes,
        // you would need to use reloadable layers or just restart with RUST_LOG=debug
    }
    
    info!("Starting Token Info Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    
    // Create providers with configuration from .env
    let providers: Vec<Arc<dyn Provider + Send + Sync>> = vec![
        Arc::new(SolanaProvider::new()),
        Arc::new(EthereumProvider::new()),
        Arc::new(BscProvider::new()),
    ];
    
    info!("Initialized providers: Solana, Ethereum, BSC");

    // CLI mode: lookup single address
    if let Some(address) = cli.token_address {
        info!("CLI mode: looking up token address: {}", address);
        let token_info = lookup_token(&providers, &address).await;
        let json = serde_json::to_string_pretty(&token_info)?;
        println!("{}", json);
        info!("Token lookup completed successfully");
        return Ok(());
    }

    // Server mode: start TCP listener
    info!("Server mode: starting on {}:{}", cli.host, cli.port);
    println!("Starting token info server on {}:{}", cli.host, cli.port);
    println!("Press Ctrl+C to stop");
    
    if let Err(e) = server::run_server(providers, &cli.host, cli.port).await {
        error!("Server error: {}", e);
        return Err(e);
    }

    Ok(())
}

async fn lookup_token(providers: &[Arc<dyn Provider + Send + Sync>], address: &str) -> TokenInfo {
    info!("Looking up token address: {}", address);
    
    for provider in providers {
        if provider.supports_address(address) {
            info!("Address supported by provider: {}", provider.name());
            match provider.get_token_info(address).await {
                Ok(info) => {
                    info!("Successfully fetched token info from {}: {:?}", provider.name(), info);
                    return info;
                },
                Err(e) => {
                    error!("Provider {} failed: {}", provider.name(), e);
                    // Continue to next provider if this one fails
                }
            }
        }
    }

    error!("No provider could resolve token address: {}", address);
    // Return placeholder if not found
    TokenInfo {
        context: address.to_string(),
        label: "?".to_string(),
        decimals: 0,
    }
}