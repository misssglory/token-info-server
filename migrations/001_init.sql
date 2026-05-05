-- Create token cache table
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
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_token_cache_address_chain ON token_cache(address, chain);
CREATE INDEX IF NOT EXISTS idx_token_cache_expires_at ON token_cache(expires_at);

-- Create cleanup function
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

-- Optional: Create a view for cache statistics
CREATE OR REPLACE VIEW cache_stats AS
SELECT 
    COUNT(*) as total_entries,
    COALESCE(SUM(hit_count), 0) as total_hits,
    COALESCE(AVG(hit_count), 0) as avg_hits_per_entry,
    COUNT(CASE WHEN expires_at < NOW() THEN 1 END) as expired_entries
FROM token_cache;

-- Optional: Create a scheduled job (requires pg_cron extension)
-- CREATE EXTENSION IF NOT EXISTS pg_cron;
-- SELECT cron.schedule('cleanup-cache', '0 * * * *', 'SELECT cleanup_expired_cache();');