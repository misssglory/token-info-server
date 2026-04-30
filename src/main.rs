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

    /// TCP port to listen on (overrides config file)
    #[arg(short, long)]
    port: Option<u16>,

    /// Host to bind to (overrides config file)
    #[arg(short, long)]
    host: Option<String>,
    
    /// Enable verbose logging (overrides config file)
    #[arg(short, long)]
    verbose: bool,
    
    /// Config file path (default: config.toml)
    #[arg(long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    
    let cli = Cli::parse();
    
    // Load configuration
    let app_config = match config::AppConfig::from_file(&cli.config) {
        Ok(cfg) => {
            info!("Loaded configuration from {}", cli.config);
            cfg
        }
        Err(e) => {
            info!("Failed to load config file ({}), using environment variables", e);
            config::AppConfig::from_env()
        }
    };
    
    // Set up logging based on config
    let log_level = if cli.verbose {
        "debug"
    } else {
        &app_config.logging.level
    };
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));
    
    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .init();
    
    info!("Starting Token Info Server v{}", env!("CARGO_PKG_VERSION"));
    info!("Log level: {}", log_level);
    
    // Use CLI overrides if provided
    let host = cli.host.as_ref().unwrap_or(&app_config.server.host);
    let port = cli.port.unwrap_or(app_config.server.port);
    
    let config = config::Config::from_env();
    
    // Initialize providers with priorities
    let mut providers: Vec<Arc<dyn TokenProvider>> = Vec::new();
    
    // ===== SOLANA PROVIDERS =====
    providers.push(Arc::new(MetadataApiProvider::new(config.solana_rpc.clone(), 1)));
    providers.push(Arc::new(PumpfunProvider::new(config.solana_rpc.clone(), 2)));
    providers.push(Arc::new(SolanaRpcProvider::new(config.solana_rpc.clone(), 4)));
    
    // ===== ETHEREUM PROVIDERS =====
    providers.push(Arc::new(EthereumProvider::new(config.ethereum_rpc.clone(), 1)));
    
    // ===== BSC PROVIDERS =====
    providers.push(Arc::new(BscProvider::new(config.bsc_rpc.clone(), 1)));
    
    info!("Initialized {} providers", providers.len());
    
    // CLI mode
    if let Some(address) = cli.token_address {
        info!("CLI mode: looking up token: {}", address);
        let token_info = lookup_token(&providers, &address).await;
        let json = serde_json::to_string_pretty(&token_info)?;
        println!("{}", json);
        
        if let Some(label) = &token_info.label {
            info!("✓ Token found: '{}' (provider: {})", label, token_info.provider);
        } else {
            info!("⚠ No label found (provider: {})", token_info.provider);
        }
        return Ok(());
    }
    
    // Server mode
    info!("Starting server on {}:{}", host, port);
    
    if let Err(e) = server::run_server(providers, host, port).await {
        error!("Server error: {}", e);
        return Err(e);
    }
    
    Ok(())
}