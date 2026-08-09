const MILLIS_PER_MINUTE: i64 = 60 * 1_000;

pub(super) const CACHE_PATH: &str = "./data/xiaomi-scale-mcp";
pub(super) const CACHE_NAMESPACE: &str = "xiaomi_scale_mcp";
pub(super) const CACHE_DATABASE: &str = "cache";
pub(super) const PROFILE_CACHE_TABLE: &str = "profile_cache";
pub(super) const WEIGHT_CACHE_TABLE: &str = "weight_cache";

pub(super) const CACHE_TTL_MS: i64 = 5 * MILLIS_PER_MINUTE;

// 7 days * 24 hours/day * 60 minutes/hour * milliseconds/minute.
pub(super) const CACHE_RETENTION_MS: i64 = 7 * 24 * 60 * MILLIS_PER_MINUTE;

pub(super) const CACHE_SCHEMA: &str = include_str!("schema.surql");
