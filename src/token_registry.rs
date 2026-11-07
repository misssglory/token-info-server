//! Durable, cross-chain token name index and Pons watcher facts.
//! OG is a historical, write-once property: unique name at first registration.
use crate::types::{TokenInfo, TokenSearchResult};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgPool, Row};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PonsEvent {
    pub event: String,
    pub ca: String,
    pub chain: Option<String>,
    pub chain_id: Option<u64>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub market_cap_usd: Option<f64>,
    pub market_cap_text: Option<String>,
    pub developer_name: Option<String>,
    pub developer_id: Option<String>,
    pub developer_profile: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
}

pub fn valid_pons_event(e: &PonsEvent) -> bool {
    matches!(e.event.as_str(), "token_new" | "token_update" | "token_recent")
        && e.chain.as_deref().unwrap_or("robinhood") == "robinhood"
        && e.chain_id.unwrap_or(4663) == 4663
        && e.ca.len() == 42 && e.ca.starts_with("0x")
        && e.ca[2..].bytes().all(|b| b.is_ascii_hexdigit())
        && e.name.as_ref().is_some_and(|n| !n.trim().is_empty() && n.len() <= 512)
        && e.market_cap_usd.is_none_or(|v| v.is_finite() && v >= 0.0)
}

pub async fn init(pool: &PgPool) -> Result<()> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS token_registry (
          chain TEXT NOT NULL,
          address TEXT NOT NULL,
          name TEXT NOT NULL,
          name_key TEXT NOT NULL,
          symbol TEXT,
          symbol_key TEXT,
          decimals SMALLINT,
          og BOOLEAN NOT NULL,
          market_cap_usd DOUBLE PRECISION,
          market_cap_text TEXT,
          developer_name TEXT,
          developer_id TEXT,
          developer_profile TEXT,
          first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          last_pons_seen_at TIMESTAMPTZ,
          PRIMARY KEY (chain, address)
        )
    "#).execute(pool).await?;
    sqlx::query("ALTER TABLE token_registry ADD COLUMN IF NOT EXISTS decimals SMALLINT")
        .execute(pool).await?;
    for statement in [
        "CREATE INDEX IF NOT EXISTS token_registry_name_prefix ON token_registry (name_key text_pattern_ops)",
        "CREATE INDEX IF NOT EXISTS token_registry_symbol_prefix ON token_registry (symbol_key text_pattern_ops)",
        "CREATE INDEX IF NOT EXISTS token_registry_chain_name ON token_registry (chain, name_key)",
    ] { sqlx::query(statement).execute(pool).await?; }
    // One-time deterministic backfill of existing cache entries. No RPC calls.
    // Older records win ties, with (chain,address) as deterministic tie-breaker.
    sqlx::query(r#"
        WITH source AS (
          SELECT chain, lower(address) address,
            left(btrim(nullif(token_info->>'long_label','')),512) name,
            nullif(token_info->>'label','') symbol,
            (token_info->>'decimals')::smallint decimals, created_at
          FROM token_cache
        ), ranked AS (
          SELECT s.*, lower(name) name_key,
            row_number() OVER (PARTITION BY lower(name) ORDER BY created_at, chain, address) rn
          FROM source s WHERE name IS NOT NULL AND name <> ''
        )
        INSERT INTO token_registry(chain,address,name,name_key,symbol,symbol_key,decimals,og,first_seen_at)
        SELECT chain,address,name,name_key,symbol,lower(symbol),decimals,
          (rn=1 AND NOT EXISTS (SELECT 1 FROM token_registry r WHERE r.name_key=ranked.name_key)), created_at
        FROM ranked
        WHERE TRUE
        ON CONFLICT (chain,address) DO NOTHING
    "#).execute(pool).await?;
    Ok(())
}

/// Serializes competing first inserts of *the same name*, across all chains.
/// The first successful DB transaction wins OG, regardless of chain.
pub async fn register(pool: &PgPool, chain: &str, address: &str, name: &str, symbol: Option<&str>, decimals: Option<u8>) -> Result<()> {
    let name = name.trim();
    if name.is_empty() { return Ok(()); }
    let name = name.chars().take(512).collect::<String>();
    let key = name.to_lowercase();
    let address = if chain == "solana" { address.to_owned() } else { address.to_ascii_lowercase() };
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(&key).execute(&mut *tx).await?;
    let present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM token_registry WHERE name_key=$1)")
        .bind(&key).fetch_one(&mut *tx).await?;
    sqlx::query(r#"
        INSERT INTO token_registry(chain,address,name,name_key,symbol,symbol_key,decimals,og)
        VALUES ($1,$2,$3,$4,$5,lower($5),$6,$7)
        ON CONFLICT (chain,address) DO UPDATE SET
          symbol = coalesce(token_registry.symbol, EXCLUDED.symbol),
          symbol_key = coalesce(token_registry.symbol_key, EXCLUDED.symbol_key),
          decimals = coalesce(token_registry.decimals, EXCLUDED.decimals)
    "#)
        .bind(chain).bind(&address).bind(&name).bind(&key).bind(symbol).bind(decimals.map(i16::from)).bind(!present)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Only Pons data is updated here; on-chain metadata and cache TTL are untouched.
/// Handles duplicate/replayed messages using observedAt to prevent stale regression.
pub async fn ingest_pons(pool: &PgPool, e: &PonsEvent) -> Result<bool> {
    if !valid_pons_event(e) { bail!("invalid Pons event or unsupported chain"); }
    let address = e.ca.to_ascii_lowercase();
    let name = e.name.as_ref().unwrap();
    register(pool, "robinhood", &address, name, e.symbol.as_deref(), None).await?;
    let received = e.observed_at.clone().unwrap_or_else(Utc::now);
    let result = sqlx::query(r#"
        UPDATE token_registry SET
          market_cap_usd=coalesce($2,market_cap_usd),
          market_cap_text=coalesce($3,market_cap_text),
          developer_name=coalesce(nullif($4,''),developer_name),
          developer_id=coalesce(nullif($5,''),developer_id),
          developer_profile=coalesce(nullif($6,''),developer_profile),
          last_pons_seen_at=$7
        WHERE chain='robinhood' AND address=$1
          AND (last_pons_seen_at IS NULL OR last_pons_seen_at <= $7)
          AND name_key IS NOT NULL
    "#)
    .bind(&address)
    .bind(e.market_cap_usd).bind(&e.market_cap_text)
    .bind(&e.developer_name).bind(&e.developer_id).bind(&e.developer_profile)
    .bind(received).execute(pool).await?;
    Ok(result.rows_affected() > 0)
}

pub async fn overlay(pool: &PgPool, token: &mut TokenInfo) -> Result<bool> {
    let address = if token.chain == "solana" { token.context.clone() } else { token.context.to_ascii_lowercase() };
    let row = sqlx::query(r#"
        SELECT name,symbol,og,market_cap_usd,market_cap_text,developer_name,developer_id,developer_profile
        FROM token_registry WHERE chain=$1 AND address=$2
    "#).bind(&token.chain).bind(&address).fetch_optional(pool).await?;
    if let Some(row) = row {
        if token.long_label.is_none() { token.long_label = Some(row.get("name")); }
        if token.label.is_none() { token.label = row.try_get("symbol")?; }
        token.og = Some(row.get("og"));
        token.market_cap_usd = row.try_get("market_cap_usd")?;
        token.market_cap_text = row.try_get("market_cap_text")?;
        token.developer_name = row.try_get("developer_name")?;
        token.developer_id = row.try_get("developer_id")?;
        token.developer_profile = row.try_get("developer_profile")?;
        return Ok(true);
    }
    Ok(false)
}

pub async fn search(pool: &PgPool, query: &str, chain: Option<&str>, limit: usize) -> Result<Vec<TokenSearchResult>> {
    let key = query.trim().to_lowercase();
    if key.is_empty() { return Ok(Vec::new()); }
    let prefix = format!("{}%", key.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let rows = sqlx::query(r#"
        SELECT address,chain,name,symbol,decimals,og,market_cap_usd,developer_name,
          CASE WHEN name_key=$1 THEN 1.0
               WHEN symbol_key=$1 THEN 0.95
               WHEN name_key LIKE $2 THEN 0.85
               ELSE 0.75 END::double precision AS score
        FROM token_registry
        WHERE ($3::text IS NULL OR chain=$3)
          AND (name_key LIKE $2 OR symbol_key LIKE $2)
        ORDER BY score DESC, first_seen_at DESC
        LIMIT $4
    "#).bind(&key).bind(&prefix).bind(chain).bind(limit.min(100) as i64)
    .fetch_all(pool).await?;
    rows.into_iter().map(|r| Ok(TokenSearchResult {
        address: r.try_get("address")?, chain: r.try_get("chain")?,
        label: r.try_get::<Option<String>,_>("symbol")?.unwrap_or_else(|| r.get("name")),
        long_label: Some(r.try_get("name")?), decimals: r.try_get::<Option<i16>,_>("decimals")?.unwrap_or(0) as u8,
        provider: "registry".into(), similarity: r.try_get("score")?,
        og: r.try_get("og")?, market_cap_usd: r.try_get("market_cap_usd")?,
        developer_name: r.try_get("developer_name")?,
    })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wrong_chain_and_invalid_ca() {
        let mut e = PonsEvent { event: "token_new".into(), ca:"0x123".into(), chain:None, chain_id:None,
            name:Some("Test".into()), symbol:None, market_cap_usd:None, market_cap_text:None,
            developer_name:None, developer_id:None, developer_profile:None, observed_at:None };
        assert!(!valid_pons_event(&e));
        e.ca = format!("0x{}", "0".repeat(40));
        assert!(valid_pons_event(&e));
        e.chain_id = Some(1);
        assert!(!valid_pons_event(&e));
    }
}
