# Pons watcher ingestion — token-info-server 0.2.0

## Start

1. Start the separate `pons-new-watcher` (v0.2.2+) on the same machine; it should expose `ws://127.0.0.1:8765/subscribe`.
2. In `config.toml`, set `[pons_watcher] enabled = true` and set `url` to the WebSocket subscription endpoint.
3. Start `token-info-server` normally, **with PostgreSQL enabled**. The registry table and B-tree prefix indexes are initialized automatically, including a one-time backfill from existing non-expired `token_cache` rows.
4. Lookups and searches use the existing newline-delimited JSON TCP protocol:

```sh
printf '%s\n' '{"type":"search","query":"Akira","limit":10}' | nc 127.0.0.1 9002 | jq .
printf '%s\n' '{"context":"0x03ba32670ca748abc87acab0d5d38e34f7461f49","chain":"robinhood"}' | nc 127.0.0.1 9002 | jq .
```

If you are using environment variables rather than `config.toml`:

```sh
PONS_WS_ENABLED=true PONS_WS_URL=ws://127.0.0.1:8765/subscribe cargo run --release -- --config /dev/null
```

## Indexed token names / OG

`token_registry` is **not cleaned up by metadata TTL expiration**. It includes all four supported chains (`solana`, `ethereum`, `bsc`, `robinhood`). Names use lowercased, trimmed keys. Name search supports **exact name, exact symbol, and prefix matches** using PostgreSQL B-tree indexes (not an unindexed `%...%` scan). Search results include `og`, `market_cap_usd`, and `developer_name` where available. Full token info lookups additionally include `market_cap_text`, `developer_id`, `developer_profile`.

`og=true` means that, **at the moment this token was first registered in this database**, no previously registered token on any of the supported chains had the same normalized full name. `og` never changes, even if later copies appear. If no full token name is known on a chain, `og` is absent rather than guessed from the ticker symbol. Existing cached rows are registered in `(created_at, chain, address)` order at initial migration; it is impossible to reconstruct historical tokens that already expired or were never cached.

For concurrent first registrations of the same name, transactions acquire a PostgreSQL advisory lock keyed by normalized name. Updates from Pons never reset `og` or overwrite on-chain metadata. A corrected developer handle or market cap is overlaid at read time, independent of cached RPC JSON.

## Async safety

- The WS consumer only awaits PostgreSQL persistence, not RPC calls. Malformed/foreign-chain events are rejected.
- `token_new` and replayed `token_recent` events may trigger Robinhood-only RPC metadata enrichment in background tasks (maximum 4 concurrent). `token_update` updates facts without RPC. Concurrent interactive lookups use per-token async singleflight locks to avoid redundant same-token RPC work.
- Replayed/stale Pons events are ignored based on `observedAt`; token updates do not reset cache expiry or OG.
- Reconnection uses exponential backoff and requests up to the last 100 watcher events. **This replay is not durable delivery**: outages exceeding the watcher's retention window can miss tokens.
- If RPC enrichment is overloaded, the token remains stored and available by name/address with `provider="pons-watcher"`; future address lookups can fill on-chain metadata.
- The existing token name search **may trigger the project's configured TCP forwarder if exactly one result matches**; that behavior is inherited from the original server.

## Deployment notes

The uploaded project did not include `Cargo.lock`. The new `tokio-tungstenite` dependency changes Cargo's vendoring hash. The Nix `packages.default` derivation uses `pkgs.lib.fakeHash` until you run `nix build -L` and paste the actual `got: sha256-...` into `flake.nix`. Alternatively, build with `cargo build --release` inside `nix develop` after Cargo resolves dependencies. Commit the generated Cargo.lock.

Any supplied `.env` with credentials is **excluded from this ZIP**; bring your own environment variables or secrets management.

## SQL inspection

```sql
SELECT chain,address,name,symbol,og,market_cap_usd,developer_name,first_seen_at,last_pons_seen_at
FROM token_registry ORDER BY first_seen_at DESC LIMIT 30;

SELECT name_key, COUNT(*) AS copies, COUNT(*) FILTER (WHERE og) AS original_marks
FROM token_registry GROUP BY name_key HAVING COUNT(*) > 1 ORDER BY copies DESC LIMIT 20;

EXPLAIN (ANALYZE, BUFFERS)
SELECT address,chain,name FROM token_registry WHERE name_key LIKE 'akira%' LIMIT 20;
```
