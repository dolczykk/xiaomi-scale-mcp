mod store;

pub(crate) use store::{CacheEntry, CacheStore};

const CACHE_PATH: &str = "./data/xiaomi-scale-mcp";
const CACHE_NAMESPACE: &str = "xiaomi_scale_mcp";
const CACHE_DATABASE: &str = "cache";
const PROFILE_CACHE_TABLE: &str = "profile_cache";
const WEIGHT_CACHE_TABLE: &str = "weight_cache";
const CACHE_RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1_000;
const CACHE_SCHEMA: &str = include_str!("schema.surql");
