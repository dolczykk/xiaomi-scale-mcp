use surrealdb::types::{Array, RecordId};

use crate::utils::current_unix_millis;

use super::consts::{PROFILE_CACHE_TABLE, WEIGHT_CACHE_TABLE};

pub(super) fn profile_record_id(xiaomi_user_id: &str) -> RecordId {
    RecordId::new(PROFILE_CACHE_TABLE, xiaomi_user_id)
}

pub(super) fn weight_record_id(xiaomi_user_id: &str, cache_key: &str) -> RecordId {
    let key = Array::from(vec![xiaomi_user_id.to_owned(), cache_key.to_owned()]);
    RecordId::new(WEIGHT_CACHE_TABLE, key)
}

pub(super) fn latest_weight_cache_key(profile_id: &str) -> String {
    format!("latest:{profile_id}")
}

pub(super) fn history_weight_cache_key(
    profile_id: &str,
    before: Option<i64>,
    page_size: u32,
) -> String {
    match before {
        Some(before) => format!("history:{profile_id}:{before}:{page_size}"),
        None => format!("history:{profile_id}:latest:{page_size}"),
    }
}

pub(super) fn now_ms() -> anyhow::Result<i64> {
    current_unix_millis().map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::{history_weight_cache_key, latest_weight_cache_key, weight_record_id};

    #[test]
    fn cache_keys_distinguish_latest_and_explicit_history_pages() {
        assert_eq!(latest_weight_cache_key("profile"), "latest:profile");
        assert_eq!(
            history_weight_cache_key("profile", None, 20),
            "history:profile:latest:20"
        );
        assert_eq!(
            history_weight_cache_key("profile", Some(123_456), 20),
            "history:profile:123456:20"
        );
    }

    #[test]
    fn compound_weight_record_ids_do_not_have_separator_collisions() {
        assert_ne!(
            weight_record_id("xiaomi:user", "cache"),
            weight_record_id("xiaomi", "user:cache")
        );
    }
}
