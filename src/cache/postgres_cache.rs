use sqlx::{postgres::{PgPoolOptions, PgConnection}, PgPool, Row};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use crate::types::TokenInfo;
use tracing::{info, warn, debug, error};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: u32,
    pub cache_ttl_seconds: i64,
}

impl CacheConfig {
    pub fn from_config(config: &crate::config::DatabaseConfig) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            database: config.database.clone(),
            username: config.username.clone(),
            password: config.password.clone(),
            max_connections: config.max_connections,
            cache_ttl_seconds: config.cache_ttl_seconds,
        }
    }
    
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }
}

#[derive(Debug, Clone)]  // Add Clone derive here
pub struct PostgresCache {
    pool: PgPool,
    ttl_seconds: i64,
}

impl PostgresCache {
    pub async fn new(config: CacheConfig) -> Result<Self> {
        info!("Connecting to PostgreSQL database...");
        
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&config.connection_string())
            .await?;
        
        // Run migrations
        Self::init_database(&pool).await?;
        
        info!("PostgreSQL cache initialized with TTL: {} seconds", config.cache_ttl_seconds);
        
        Ok(Self {
            pool,
            ttl_seconds: config.cache_ttl_seconds,
        })
    }
    
    async fn init_database(pool: &PgPool) -> Result<()> {
        // Create cache table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS token_cache (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                address TEXT NOT NULL,
                chain TEXT NOT NULL,
                token_info JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ NOT NULL,
                hit_count BIGINT DEFAULT 0,
                UNIQUE(address, chain)
            )
            "#,
        )
        .execute(pool)
        .await?;
        
        // Create index on address and chain
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_token_cache_address_chain 
            ON token_cache(address, chain)
            "#,
        )
        .execute(pool)
        .await?;
        
        // Create index on expires_at for cleanup
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_token_cache_expires_at 
            ON token_cache(expires_at)
            "#,
        )
        .execute(pool)
        .await?;
        
        // Create stored procedure for cleanup
        sqlx::query(
            r#"
            CREATE OR REPLACE FUNCTION cleanup_expired_cache()
            RETURNS INTEGER AS $$
            DECLARE
                deleted_count INTEGER;
            BEGIN
                DELETE FROM token_cache
                WHERE expires_at < NOW()
                RETURNING COUNT(*) INTO deleted_count;
                
                RETURN deleted_count;
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .execute(pool)
        .await?;
        
        info!("Database tables and indexes created successfully");
        Ok(())
    }
    
    pub async fn get(&self, address: &str, chain: &str) -> Option<TokenInfo> {
        let query = sqlx::query(
            r#"
            UPDATE token_cache 
            SET hit_count = hit_count + 1
            WHERE address = $1 AND chain = $2 AND expires_at > NOW()
            RETURNING token_info, expires_at, hit_count
            "#,
        )
        .bind(address)
        .bind(chain);
        
        match query.fetch_optional(&self.pool).await {
            Ok(Some(row)) => {
                let token_info: serde_json::Value = row.get("token_info");
                let expires_at: DateTime<Utc> = row.get("expires_at");
                let hit_count: i64 = row.get("hit_count");
                
                match serde_json::from_value::<TokenInfo>(token_info) {
                    Ok(info) => {
                        debug!("Cache hit for {} on {} (hit_count: {}, expires_at: {})", 
                               address, chain, hit_count, expires_at);
                        Some(info)
                    }
                    Err(e) => {
                        error!("Failed to deserialize cached token info: {}", e);
                        None
                    }
                }
            }
            Ok(None) => {
                debug!("Cache miss for {} on {}", address, chain);
                None
            }
            Err(e) => {
                error!("Database error during cache lookup: {}", e);
                None
            }
        }
    }
    
    pub async fn set(&self, address: &str, chain: &str, token_info: &TokenInfo) -> Result<()> {
        let expires_at = Utc::now() + Duration::seconds(self.ttl_seconds);
        let token_info_json = serde_json::to_value(token_info)?;
        
        let query = sqlx::query(
            r#"
            INSERT INTO token_cache (address, chain, token_info, expires_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (address, chain) 
            DO UPDATE SET 
                token_info = EXCLUDED.token_info,
                expires_at = EXCLUDED.expires_at,
                updated_at = NOW()
            "#,
        )
        .bind(address)
        .bind(chain)
        .bind(&token_info_json)
        .bind(expires_at);
        
        query.execute(&self.pool).await?;
        debug!("Cached token info for {} on {} (expires in {} seconds)", 
               address, chain, self.ttl_seconds);
        
        Ok(())
    }
    
    pub async fn delete(&self, address: &str, chain: &str) -> Result<()> {
        let query = sqlx::query(
            r#"
            DELETE FROM token_cache
            WHERE address = $1 AND chain = $2
            "#,
        )
        .bind(address)
        .bind(chain);
        
        query.execute(&self.pool).await?;
        debug!("Deleted cache entry for {} on {}", address, chain);
        
        Ok(())
    }
    
    pub async fn cleanup_expired(&self) -> Result<i64> {
        let result = sqlx::query(
            r#"
            SELECT cleanup_expired_cache() as deleted_count
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        
        let deleted_count: i64 = result.get("deleted_count");
        
        if deleted_count > 0 {
            info!("Cleaned up {} expired cache entries", deleted_count);
        }
        
        Ok(deleted_count)
    }
    
    pub async fn get_stats(&self) -> Result<CacheStats> {
        let stats = sqlx::query_as::<_, CacheStatsRow>(
            r#"
            SELECT 
                COUNT(*) as total_entries,
                COALESCE(SUM(hit_count), 0) as total_hits,
                COALESCE(AVG(hit_count), 0) as avg_hits_per_entry,
                COUNT(CASE WHEN expires_at < NOW() THEN 1 END) as expired_entries
            FROM token_cache
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(CacheStats {
            total_entries: stats.total_entries,
            total_hits: stats.total_hits,
            avg_hits_per_entry: stats.avg_hits_per_entry,
            expired_entries: stats.expired_entries,
        })
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub total_entries: i64,
    pub total_hits: i64,
    pub avg_hits_per_entry: f64,
    pub expired_entries: i64,
}

#[derive(sqlx::FromRow)]
struct CacheStatsRow {
    total_entries: i64,
    total_hits: i64,
    avg_hits_per_entry: f64,
    expired_entries: i64,
}