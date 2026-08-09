use anyhow::Context;
use serde::Serialize;
use serde::de::DeserializeOwned;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::types::{RecordId, SurrealValue};

use super::consts::{
    CACHE_DATABASE, CACHE_NAMESPACE, CACHE_PATH, CACHE_RETENTION_MS, CACHE_SCHEMA,
};
use super::utils::{now_ms, profile_record_id, weight_record_id};

#[derive(Debug, Clone, SurrealValue)]
struct StoredCacheEntry {
    fetched_at_ms: i64,
    payload: String,
}

#[derive(Debug, Clone, SurrealValue)]
struct ProfileCacheRecord {
    id: Option<RecordId>,
    xiaomi_user_id: String,
    fetched_at_ms: i64,
    payload: String,
}

#[derive(Debug, Clone, SurrealValue)]
struct WeightCacheRecord {
    id: Option<RecordId>,
    xiaomi_user_id: String,
    cache_key: String,
    profile_id: String,
    fetched_at_ms: i64,
    payload: String,
}

impl From<ProfileCacheRecord> for StoredCacheEntry {
    fn from(record: ProfileCacheRecord) -> Self {
        Self {
            fetched_at_ms: record.fetched_at_ms,
            payload: record.payload,
        }
    }
}

impl From<WeightCacheRecord> for StoredCacheEntry {
    fn from(record: WeightCacheRecord) -> Self {
        Self {
            fetched_at_ms: record.fetched_at_ms,
            payload: record.payload,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CacheEntry<T> {
    pub(crate) fetched_at_ms: i64,
    pub(crate) value: T,
}

#[derive(Clone)]
pub(crate) struct CacheDal {
    db: Surreal<Db>,
}

impl CacheDal {
    pub(crate) async fn open() -> anyhow::Result<Self> {
        match Self::open_disk().await {
            Ok(cache) => Ok(cache),
            Err(error) => {
                log::warn!(
                    "Failed to initialize SurrealKV cache at {CACHE_PATH}: {error:#}; using in-memory cache"
                );
                Self::open_memory()
                    .await
                    .context("failed to initialize fallback in-memory cache")
            }
        }
    }

    async fn open_disk() -> anyhow::Result<Self> {
        let database = Surreal::new::<SurrealKv>(CACHE_PATH)
            .await
            .context("failed to open embedded SurrealKV")?;

        Self::prepare(database).await
    }

    async fn open_memory() -> anyhow::Result<Self> {
        let database = Surreal::new::<Mem>(())
            .await
            .context("failed to open in-memory SurrealDB")?;

        Self::prepare(database).await
    }

    async fn prepare(database: Surreal<Db>) -> anyhow::Result<Self> {
        database
            .use_ns(CACHE_NAMESPACE)
            .use_db(CACHE_DATABASE)
            .await
            .context("failed to select cache namespace and database")?;

        database
            .query(CACHE_SCHEMA)
            .await
            .context("failed to define cache schema")?
            .check()
            .context("cache schema contains an invalid statement")?;

        let cache = Self { db: database };
        cache.cleanup_expired(now_ms()?).await?;

        Ok(cache)
    }

    async fn cleanup_expired(&self, now_ms: i64) -> anyhow::Result<()> {
        let cutoff_ms = now_ms.saturating_sub(CACHE_RETENTION_MS);
        self.db
            .query(
                "DELETE profile_cache WHERE fetched_at_ms < $cutoff_ms; \
                 DELETE weight_cache WHERE fetched_at_ms < $cutoff_ms;",
            )
            .bind(("cutoff_ms", cutoff_ms))
            .await
            .context("failed to clean expired cache records")?
            .check()
            .context("cache cleanup contains an invalid statement")?;

        Ok(())
    }

    pub(crate) async fn load_profiles<T: DeserializeOwned>(
        &self,
        xiaomi_user_id: &str,
    ) -> anyhow::Result<Option<CacheEntry<T>>> {
        let stored: Option<ProfileCacheRecord> = self
            .db
            .select(profile_record_id(xiaomi_user_id))
            .await
            .context("failed to load cached Xiaomi profiles")?;

        deserialize_entry(stored.map(StoredCacheEntry::from))
    }

    pub(crate) async fn save_profiles<T: Serialize + ?Sized>(
        &self,
        xiaomi_user_id: &str,
        profiles: &T,
        fetched_at_ms: i64,
    ) -> anyhow::Result<()> {
        let payload = serde_json::to_string(profiles).context("failed to serialize profiles")?;
        let record = ProfileCacheRecord {
            id: None,
            xiaomi_user_id: xiaomi_user_id.to_owned(),
            fetched_at_ms,
            payload,
        };

        let _: Option<ProfileCacheRecord> = self
            .db
            .upsert(profile_record_id(xiaomi_user_id))
            .content(record)
            .await
            .context("failed to save cached Xiaomi profiles")?;

        Ok(())
    }

    pub(crate) async fn load_weights<T: DeserializeOwned>(
        &self,
        xiaomi_user_id: &str,
        cache_key: &str,
    ) -> anyhow::Result<Option<CacheEntry<T>>> {
        let stored: Option<WeightCacheRecord> = self
            .db
            .select(weight_record_id(xiaomi_user_id, cache_key))
            .await
            .context("failed to load cached Xiaomi weights")?;

        deserialize_entry(stored.map(StoredCacheEntry::from))
    }

    pub(crate) async fn save_weights<T: Serialize + ?Sized>(
        &self,
        xiaomi_user_id: &str,
        cache_key: &str,
        profile_id: &str,
        measurements: &T,
        fetched_at_ms: i64,
    ) -> anyhow::Result<()> {
        let payload =
            serde_json::to_string(measurements).context("failed to serialize measurements")?;
        let record = WeightCacheRecord {
            id: None,
            xiaomi_user_id: xiaomi_user_id.to_owned(),
            cache_key: cache_key.to_owned(),
            profile_id: profile_id.to_owned(),
            fetched_at_ms,
            payload,
        };

        let _: Option<WeightCacheRecord> = self
            .db
            .upsert(weight_record_id(xiaomi_user_id, cache_key))
            .content(record)
            .await
            .context("failed to save cached Xiaomi weights")?;

        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn in_memory() -> anyhow::Result<Self> {
        Self::open_memory().await
    }
}

fn deserialize_entry<T: DeserializeOwned>(
    stored: Option<StoredCacheEntry>,
) -> anyhow::Result<Option<CacheEntry<T>>> {
    stored
        .map(|stored| {
            let value = serde_json::from_str(&stored.payload)
                .context("failed to deserialize cached JSON payload")?;
            Ok(CacheEntry {
                fetched_at_ms: stored.fetched_at_ms,
                value,
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{CacheDal, ProfileCacheRecord, WeightCacheRecord};
    use crate::dal::consts::{CACHE_RETENTION_MS, PROFILE_CACHE_TABLE, WEIGHT_CACHE_TABLE};

    #[tokio::test]
    async fn profile_cache_round_trips_and_is_partitioned_by_xiaomi_user() {
        let cache = CacheDal::in_memory().await.unwrap();
        let profiles = vec!["profile-1".to_string()];

        cache
            .save_profiles("xiaomi-user-1", &profiles, 123_456)
            .await
            .unwrap();

        let stored = cache
            .load_profiles::<Vec<String>>("xiaomi-user-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.value, profiles);
        assert!(
            cache
                .load_profiles::<Vec<String>>("xiaomi-user-2")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn weight_cache_round_trips_and_is_partitioned_by_xiaomi_user() {
        let cache = CacheDal::in_memory().await.unwrap();
        let measurements = vec!["measurement-1".to_string()];

        cache
            .save_weights(
                "xiaomi-user-1",
                "latest:profile",
                "profile",
                &measurements,
                123_456,
            )
            .await
            .unwrap();

        let stored = cache
            .load_weights::<Vec<String>>("xiaomi-user-1", "latest:profile")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.value, measurements);
        assert!(
            cache
                .load_weights::<Vec<String>>("xiaomi-user-2", "latest:profile")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn repeated_profile_saves_update_one_record() {
        let cache = CacheDal::in_memory().await.unwrap();

        cache
            .save_profiles("xiaomi-user-1", &["profile-1"], 123_456)
            .await
            .unwrap();
        cache
            .save_profiles("xiaomi-user-1", &["profile-2"], 123_457)
            .await
            .unwrap();

        let records: Vec<ProfileCacheRecord> = cache.db.select(PROFILE_CACHE_TABLE).await.unwrap();
        let stored = cache
            .load_profiles::<Vec<String>>("xiaomi-user-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(stored.value, vec!["profile-2"]);
        assert_eq!(stored.fetched_at_ms, 123_457);
    }

    #[tokio::test]
    async fn repeated_weight_saves_update_one_record() {
        let cache = CacheDal::in_memory().await.unwrap();

        cache
            .save_weights(
                "xiaomi-user-1",
                "latest:profile",
                "profile",
                &["measurement-1"],
                123_456,
            )
            .await
            .unwrap();
        cache
            .save_weights(
                "xiaomi-user-1",
                "latest:profile",
                "profile",
                &["measurement-2"],
                123_457,
            )
            .await
            .unwrap();

        let records: Vec<WeightCacheRecord> = cache.db.select(WEIGHT_CACHE_TABLE).await.unwrap();
        let stored = cache
            .load_weights::<Vec<String>>("xiaomi-user-1", "latest:profile")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(stored.value, vec!["measurement-2"]);
        assert_eq!(stored.fetched_at_ms, 123_457);
    }

    #[tokio::test]
    async fn cleanup_removes_records_older_than_retention() {
        let cache = CacheDal::in_memory().await.unwrap();
        let now_ms = 10 * CACHE_RETENTION_MS;
        let expired_at = now_ms - CACHE_RETENTION_MS - 1;

        cache
            .save_profiles("xiaomi-user-1", &["profile"], expired_at)
            .await
            .unwrap();
        cache
            .save_weights(
                "xiaomi-user-1",
                "latest:profile",
                "profile",
                &["measurement"],
                expired_at,
            )
            .await
            .unwrap();
        cache.cleanup_expired(now_ms).await.unwrap();

        assert!(
            cache
                .load_profiles::<Vec<String>>("xiaomi-user-1")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            cache
                .load_weights::<Vec<String>>("xiaomi-user-1", "latest:profile")
                .await
                .unwrap()
                .is_none()
        );
    }
}
