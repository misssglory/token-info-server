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
    
    // Initialize providers with priorities (lower number = higher priority)
    let mut providers: Vec<Arc<dyn TokenProvider>> = Vec::new();
    
    // ===== SOLANA PROVIDERS =====
    // Priority 1: Metaplex metadata provider (fetches token names from on-chain metadata)
    providers.push(Arc::new(MetadataApiProvider::new(config.solana_rpc.clone(), 1)));
    
    // Priority 2: Pump.fun provider (specialized for Pump.fun tokens)
    providers.push(Arc::new(PumpfunProvider::new(config.solana_rpc.clone(), 2)));
    
    // Priority 3: Basic Solana RPC provider (fallback - returns decimals only, no name)
    providers.push(Arc::new(SolanaRpcProvider::new(config.solana_rpc.clone(), 3)));
    
    // ===== ETHEREUM PROVIDERS =====
    providers.push(Arc::new(EthereumProvider::new(config.ethereum_rpc.clone(), 1)));
    
    // ===== BSC PROVIDERS =====
    providers.push(Arc::new(BscProvider::new(config.bsc_rpc.clone(), 1)));
    
    info!("Initialized {} providers", providers.len());
    info!("Provider order: Metadata(1), Pumpfun(2), SolanaRPC(3), Ethereum(1), BSC(1)");
    
    // CLI mode
    if let Some(address) = cli.token_address {
        info!("CLI mode: looking up token address: {}", address);
        let token_info = lookup_token(&providers, &address).await;
        let json = serde_json::to_string_pretty(&token_info)?;
        println!("{}", json);
        
        // Show which provider was used
        if let Some(label) = &token_info.label {
            info!("✓ Token name found: '{}' using provider: {}", label, token_info.provider);
        } else {
            info!("⚠ No token name found, using provider: {}", token_info.provider);
        }
        
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
