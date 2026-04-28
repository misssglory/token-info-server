mod providers;
mod server;
mod types;
mod lookup;
mod config;

use clap::Parser;
use dotenv::dotenv;
use providers::*;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{fmt, EnvFilter};
use crate::lookup::lookup_token;

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
    dotenv().ok();
    
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
    let config = config::Config::from_env();
    
    if cli.verbose {
        tracing::info!("Verbose logging enabled");
    }
    
    info!("Starting Token Info Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    
    // Initialize providers with priorities
    let mut providers: Vec<Arc<dyn TokenProvider>> = Vec::new();
    
    // Solana providers (priorities defined in config)
    providers.push(Arc::new(PumpfunProvider::new(config.solana_rpc.clone(), 1)));
    providers.push(Arc::new(SolanaRpcProvider::new(config.solana_rpc.clone(), 2)));
    
    // Ethereum providers
    providers.push(Arc::new(EthereumProvider::new(config.ethereum_rpc.clone(), 1)));
    
    // BSC providers
    providers.push(Arc::new(BscProvider::new(config.bsc_rpc.clone(), 1)));
    
    info!("Initialized {} providers", providers.len());
    
    // CLI mode
    if let Some(address) = cli.token_address {
        info!("CLI mode: looking up token address: {}", address);
        let token_info = lookup_token(&providers, &address).await;
        let json = serde_json::to_string_pretty(&token_info)?;
        println!("{}", json);
        info!("Token lookup completed successfully");
        return Ok(());
    }
    
    // Server mode
    info!("Server mode: starting on {}:{}", cli.host, cli.port);
    println!("Starting token info server on {}:{}", cli.host, cli.port);
    println!("Press Ctrl+C to stop");
    
    if let Err(e) = server::run_server(providers, &cli.host, cli.port).await {
        error!("Server error: {}", e);
        return Err(e);
    }
    
    Ok(())
}