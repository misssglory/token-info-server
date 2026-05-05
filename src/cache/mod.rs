mod postgres_cache;

pub use postgres_cache::PostgresCache;
// Make CacheConfig available through the module
pub use postgres_cache::CacheConfig;