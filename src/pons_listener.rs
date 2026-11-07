//! Asynchronous Pons WebSocket consumer. Persist before launching bounded RPC enrichment.
use crate::{cache::PostgresCache, lookup::lookup_token, providers::TokenProvider, token_registry::{self, PonsEvent}};
use futures::{SinkExt, StreamExt};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::sync::{Mutex, Semaphore};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

pub fn start(url: String, cache: Arc<PostgresCache>, providers: Vec<Arc<dyn TokenProvider>>) {
    tokio::spawn(async move {
        let inflight = Arc::new(Mutex::new(HashSet::<String>::new()));
        let limiter = Arc::new(Semaphore::new(4));
        let mut retry_seconds = 1u64;
        loop {
            match connect_async(&url).await {
                Ok((socket, _)) => {
                    info!(%url, "Connected to Pons watcher");
                    retry_seconds = 1;
                    let (mut sink, mut stream) = socket.split();
                    // Recover up to 100 recent events after short interruptions.
                    if let Err(err) = sink.send(Message::Text(r#"{"action":"recent","limit":100}"#.into())).await {
                        warn!(%err, "Pons replay request failed");
                    }
                    while let Some(message) = stream.next().await {
                        let message = match message {
                            Ok(Message::Text(s)) => s,
                            Ok(Message::Binary(bytes)) => match String::from_utf8(bytes.to_vec()) { Ok(s) => s.into(), Err(_) => continue },
                            Ok(Message::Close(_)) => break,
                            Ok(_) => continue,
                            Err(err) => { warn!(%err, "Pons WebSocket read failed"); break; }
                        };
                        let event: PonsEvent = match serde_json::from_str(&message) {
                            Ok(e) => e,
                            Err(_) => continue, // hello, pong, non-token events
                        };
                        if !token_registry::valid_pons_event(&event) { continue; }
                        let address = event.ca.to_ascii_lowercase();
                        // DB writes are awaited so ingestion order is preserved, but no RPC is awaited here.
                        match token_registry::ingest_pons(cache.pool(), &event).await {
                            Ok(false) => { debug!(%address, "Ignored stale Pons event"); continue; },
                            Err(err) => { warn!(%address, %err, "Pons event persistence failed"); break; },
                            _ => {}
                        }
                        if !matches!(event.event.as_str(), "token_new" | "token_recent") { continue; }
                        let addr = address.clone();
                        let cache = cache.clone();
                        let providers = providers.clone();
                        let inflight = inflight.clone();
                        {
                            let mut active = inflight.lock().await;
                            if !active.insert(addr.clone()) { continue; }
                        }
                        // Never accumulate unbounded queued RPC tasks during launch spikes.
                        // Dropped enrichment can be fulfilled by normal address lookups.
                        let permit = match limiter.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                inflight.lock().await.remove(&addr);
                                debug!(%addr, "RPC enrichment busy; event safely stored in registry");
                                continue;
                            }
                        };
                        tokio::spawn(async move {
                            let _permit = permit;
                            // Idempotency: skip RPC when non-expiring registry data already has an
                            // on-chain metadata cache hit. Never guess chain from a CA.
                            let already_cached = cache.get(&addr, "robinhood").await;
                            if !matches!(already_cached, Ok(Some(ref t)) if t.provider != "pons-watcher" && t.provider != "none") {
                                let result = tokio::time::timeout(Duration::from_secs(25),
                                    lookup_token(&providers, &addr, Some("robinhood"), Some(&cache), None)).await;
                                if let Err(err) = &result { warn!(%addr, %err, "Pons RPC enrichment timeout"); }
                                if let Ok(Err(err)) = result { warn!(%addr, %err, "Pons RPC enrichment failed"); }
                            }
                            inflight.lock().await.remove(&addr);
                        });
                    }
                }
                Err(err) => warn!(%url, %err, "Pons watcher unavailable"),
            }
            tokio::time::sleep(Duration::from_secs(retry_seconds)).await;
            retry_seconds = (retry_seconds * 2).min(30);
        }
    });
}
