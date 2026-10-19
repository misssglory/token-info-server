mod cache;
mod config;
mod lookup;
mod providers;
mod server;
mod tcp_forwarder;
mod token_index;
mod types;

use crate::cache::PostgresCache;
use crate::lookup::lookup_token;
use clap::Parser;
use dotenv::dotenv;
use providers::*;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
// Remove this line - CacheConfig is now exported from cache module
// use crate::cache::postgres_cache::CacheConfig;
use crate::cache::CacheConfig; // Use this instead
use crate::config::AppConfig;
use crate::tcp_forwarder::{TcpForwarder, TcpForwarderConfig};

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

    /// Force chain (solana, ethereum, bsc, robinhood); otherwise auto-detect
    #[arg(long)]
    chain: Option<String>,

    /// Skip cache for this lookup
    #[arg(long)]
    no_cache: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let cli = Cli::parse();

    // Load configuration
    let app_config = match AppConfig::from_file(&cli.config) {
        Ok(cfg) => {
            info!("Loaded configuration from {}", cli.config);
            cfg
        }
        Err(e) => {
            info!(
                "Failed to load config file ({}), using environment variables",
                e
            );
            AppConfig::from_env()
        }
    };

    // Set up logging
    let log_level = if cli.verbose {
        "debug"
    } else {
        &app_config.logging.level
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Token Info Server v{}", env!("CARGO_PKG_VERSION"));
    info!("Log level: {}", log_level);

    // Initialize PostgreSQL cache
    let cache = if !cli.no_cache {
        let cache_config = CacheConfig::from_config(&app_config.database);
        let c = PostgresCache::new(cache_config).await
            .map_err(|e| anyhow::anyhow!("Cache enabled but PostgreSQL initialization failed: {e:#}. Use --no-cache to explicitly bypass it"))?;
        info!("PostgreSQL cache initialized successfully");
        let cache_for_cleanup = c.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                if let Err(e) = cache_for_cleanup.cleanup_expired().await {
                    error!("Cache cleanup failed: {}", e);
                }
            }
        });
        Some(Arc::new(c))
    } else {
        info!("Cache disabled via CLI flag");
        None
    };

    // Initialize TCP forwarder
    let forwarder = if app_config.tcp_endpoint.enabled {
        let forwarder_config = TcpForwarderConfig::from_config(&app_config.tcp_endpoint);
        let forwarder = Arc::new(TcpForwarder::new(forwarder_config));

        // Try to connect
        if let Err(e) = forwarder.connect().await {
            error!("Failed to connect to TCP endpoint: {}", e);
        } else {
            info!("TCP forwarder initialized and connected");
        }

        Some(forwarder)
    } else {
        info!("TCP forwarder disabled");
        None
    };

    // Use CLI overrides
    let host = cli.host.as_ref().unwrap_or(&app_config.server.host);
    let port = cli.port.unwrap_or(app_config.server.port);

    let config = config::Config::from_env();

    // Initialize providers
    let mut providers: Vec<Arc<dyn TokenProvider>> = Vec::new();

    // SOLANA PROVIDERS
    providers.push(Arc::new(MetadataApiProvider::new(
        config.solana_rpc.clone(),
        1,
    )));
    providers.push(Arc::new(PumpfunProvider::new(config.solana_rpc.clone(), 2)));
    providers.push(Arc::new(SolanaRpcProvider::new(
        config.solana_rpc.clone(),
        4,
    )));
    // Add to provider initialization
    providers.push(Arc::new(DexScreenerProvider::new(1)));
    if let Some(helius_key) = config.helius_api_key {
        providers.push(Arc::new(HeliusProvider::new(helius_key, 2)));
    }
    // ETHEREUM PROVIDERS
    providers.push(Arc::new(EthereumProvider::new(
        config.ethereum_rpc.clone(),
        1,
    )));

    // BSC PROVIDERS
    providers.push(Arc::new(BscProvider::new(config.bsc_rpc.clone(), 1)));

    // On-chain ERC-20 metadata for all EVM networks.
    providers.push(Arc::new(EvmRpcProvider::new("robinhood", config.robinhood_rpc.clone(), 1)));
    providers.push(Arc::new(EvmRpcProvider::new("ethereum", config.ethereum_rpc.clone(), 3)));
    providers.push(Arc::new(EvmRpcProvider::new("bsc", config.bsc_rpc.clone(), 3)));

    info!("Initialized {} providers", providers.len());

    // Print cache stats if available
    if let Some(cache) = &cache {
        match cache.get_stats().await {
            Ok(stats) => {
                info!(
                    "Cache stats: {} entries, {} total hits",
                    stats.total_entries, stats.total_hits
                );
            }
            Err(e) => {
                error!("Failed to get cache stats: {}", e);
            }
        }
    }

    // CLI mode
    if let Some(address) = cli.token_address {
        info!("CLI mode: looking up token: {}", address);
        let token_info = lookup_token(
            &providers,
            &address,
            cli.chain.as_deref(),
            cache.as_ref().map(|c| c.as_ref()),
            forwarder.as_ref().map(|f| f.as_ref()),
        )
        .await?;
        let json = serde_json::to_string_pretty(&token_info)?;
        println!("{}", json);

        if let Some(label) = &token_info.label {
            info!(
                "✓ Token found: '{}' (provider: {})",
                label, token_info.provider
            );
        } else {
            info!("⚠ No label found (provider: {})", token_info.provider);
        }
        return Ok(());
    }

    // Server mode
    info!("Starting server on {}:{}", host, port);

    if let Err(e) = server::run_server(providers, cache, forwarder, host, port).await {
        error!("Server error: {}", e);
        return Err(e);
    }

    Ok(())
}
