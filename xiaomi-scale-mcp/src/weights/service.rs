use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use xiaomi_client::home::account::{GetWeightAccountsRequest, WeightAccount};
use xiaomi_client::home::devices::{DeviceItem, GetDevicesRequest};
use xiaomi_client::home::weight::{WeightDataRecord, WeightUserDataRequest};
use xiaomi_client::utils::local_timezone_offset_seconds;

use crate::cache::{CacheEntry, CacheStore};
use crate::session::{AuthenticatedXiaomi, XiaomiSession};
use crate::time::current_unix_millis;

use super::types::{MCPWeightProfile, MCPWeightResult, ProfileContext};

pub(crate) const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 100;
const CACHE_TTL_MS: i64 = 5 * 60 * 1_000;

#[derive(Clone)]
pub(crate) struct WeightService {
    cache: CacheStore,
    xiaomi_session: Arc<XiaomiSession>,
}

impl WeightService {
    pub(crate) fn new(cache: CacheStore, xiaomi_session: Arc<XiaomiSession>) -> Self {
        Self {
            cache,
            xiaomi_session,
        }
    }

    pub(crate) async fn profiles(&self) -> anyhow::Result<Vec<MCPWeightProfile>> {
        let xiaomi = self.xiaomi_session.authenticated().await?;
        Ok(self
            .profile_contexts(&xiaomi)
            .await?
            .iter()
            .map(weight_profile)
            .collect())
    }

    pub(crate) async fn latest_weight(&self, profile_id: &str) -> anyhow::Result<MCPWeightResult> {
        let xiaomi = self.xiaomi_session.authenticated().await?;
        let measurements = self
            .weight_page(
                &xiaomi,
                profile_id,
                None,
                1,
                &latest_cache_key(profile_id),
                false,
            )
            .await?;

        measurements
            .into_iter()
            .next()
            .context("no weight measurements found for profile")
    }

    pub(crate) async fn historical_weights(
        &self,
        profile_id: &str,
        before: Option<i64>,
        page_size: u32,
    ) -> anyhow::Result<Vec<MCPWeightResult>> {
        let xiaomi = self.xiaomi_session.authenticated().await?;
        self.weight_page(
            &xiaomi,
            profile_id,
            before,
            page_size,
            &history_cache_key(profile_id, before, page_size),
            true,
        )
        .await
    }

    async fn profile_contexts(
        &self,
        xiaomi: &AuthenticatedXiaomi,
    ) -> anyhow::Result<Vec<ProfileContext>> {
        let now_ms = current_unix_millis()?;
        let cached = self
            .cache
            .load_profiles::<Vec<ProfileContext>>(xiaomi.user_id())
            .await
            .unwrap_or_else(|error| {
                log::warn!("Failed to read Xiaomi profile cache: {error:#}");
                None
            });

        if let Some(entry) = cached.as_ref().filter(|entry| entry.is_fresh(now_ms)) {
            return Ok(entry.value.clone());
        }

        let profiles = match self.fetch_profiles(xiaomi).await {
            Ok(profiles) => profiles,
            Err(error) => {
                if let Some(entry) = cached {
                    log::warn!(
                        "Xiaomi profile refresh failed; returning stale cached data: {error:#}"
                    );
                    return Ok(entry.value);
                }
                return Err(error);
            }
        };

        if let Err(error) = self
            .cache
            .save_profiles(xiaomi.user_id(), &profiles, now_ms)
            .await
        {
            log::warn!("Failed to update Xiaomi profile cache: {error:#}");
        }

        Ok(profiles)
    }

    async fn weight_page(
        &self,
        xiaomi: &AuthenticatedXiaomi,
        profile_id: &str,
        before: Option<i64>,
        page_size: u32,
        cache_key: &str,
        empty_page_is_valid: bool,
    ) -> anyhow::Result<Vec<MCPWeightResult>> {
        let now_ms = current_unix_millis()?;
        let cached = self
            .cache
            .load_weights::<Vec<MCPWeightResult>>(xiaomi.user_id(), cache_key)
            .await
            .unwrap_or_else(|error| {
                log::warn!("Failed to read Xiaomi weight cache: {error:#}");
                None
            });

        if let Some(entry) = cached
            .as_ref()
            .filter(|entry| entry.is_fresh(now_ms))
            .filter(|entry| empty_page_is_valid || !entry.value.is_empty())
        {
            return Ok(entry.value.clone());
        }

        let measurements = match self
            .fetch_weight_page(xiaomi, profile_id, before.unwrap_or(now_ms), page_size)
            .await
            .and_then(|measurements| {
                if empty_page_is_valid || !measurements.is_empty() {
                    Ok(measurements)
                } else {
                    Err(anyhow::anyhow!("no weight measurements found for profile"))
                }
            }) {
            Ok(measurements) => measurements,
            Err(error) => {
                if let Some(entry) =
                    cached.filter(|entry| empty_page_is_valid || !entry.value.is_empty())
                {
                    log::warn!(
                        "Xiaomi weight refresh failed; returning stale cached data: {error:#}"
                    );
                    return Ok(entry.value);
                }
                return Err(error);
            }
        };

        if let Err(error) = self
            .cache
            .save_weights(
                xiaomi.user_id(),
                cache_key,
                profile_id,
                &measurements,
                now_ms,
            )
            .await
        {
            log::warn!("Failed to update Xiaomi weight cache: {error:#}");
        }

        Ok(measurements)
    }

    async fn fetch_profiles(
        &self,
        xiaomi: &AuthenticatedXiaomi,
    ) -> anyhow::Result<Vec<ProfileContext>> {
        let client = xiaomi.client().await?;
        let devices = client
            .get_devices(&GetDevicesRequest::default())
            .await
            .context("failed to discover Xiaomi Home devices")?;
        let mut profiles = Vec::new();

        for scale in devices
            .result
            .list
            .into_iter()
            .filter(|device| device.model.starts_with("yunmai.scales."))
        {
            let request =
                GetWeightAccountsRequest::new(scale.user_id.to_string(), scale.device_id.clone());
            let accounts = client
                .get_weight_accounts(&request, &scale.model)
                .await
                .with_context(|| {
                    format!(
                        "failed to discover weight profiles for scale {}",
                        scale.name
                    )
                })?;

            profiles.extend(
                accounts
                    .result
                    .into_iter()
                    .map(|account| profile_context(&scale, account)),
            );
        }

        log::info!("Discovered {} Xiaomi weight profiles", profiles.len());
        Ok(profiles)
    }

    async fn fetch_weight_page(
        &self,
        xiaomi: &AuthenticatedXiaomi,
        profile_id: &str,
        before_ms: i64,
        page_size: u32,
    ) -> anyhow::Result<Vec<MCPWeightResult>> {
        let profiles = self.profile_contexts(xiaomi).await?;
        let profile = profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
            .with_context(|| unknown_profile_error(profile_id))?;
        let request = weight_data_request(profile, before_ms, page_size);
        let response = xiaomi
            .client()
            .await?
            .get_weight_user_data(&request)
            .await
            .context("failed to fetch Xiaomi weight measurements")?;

        response
            .result
            .into_iter()
            .map(|record| weight_measurement(profile_id, record).map_err(anyhow::Error::msg))
            .collect()
    }
}

impl<T> CacheEntry<T> {
    fn is_fresh(&self, now_ms: i64) -> bool {
        now_ms >= self.fetched_at_ms && now_ms.saturating_sub(self.fetched_at_ms) <= CACHE_TTL_MS
    }
}

pub(crate) fn validate_page_size(page_size: Option<u32>) -> Result<u32, String> {
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
        return Err("page_size must be between 1 and 100".to_string());
    }
    Ok(page_size)
}

fn profile_context(scale: &DeviceItem, account: WeightAccount) -> ProfileContext {
    ProfileContext {
        profile_id: format!("{}:{}", scale.device_id, account.account_id),
        account_id: account.account_id,
        user_id: scale.user_id.to_string(),
        device_id: scale.device_id.clone(),
        scale_name: scale.name.clone(),
        scale_model: scale.model.clone(),
        name: account.name,
        height: account.height,
        weight_target: account.weight_target,
        last_weight_update_time: account.weight_update_time,
    }
}

fn weight_data_request(
    profile: &ProfileContext,
    before: i64,
    page_size: u32,
) -> WeightUserDataRequest {
    WeightUserDataRequest::new(
        profile.scale_model.clone(),
        profile.user_id.clone(),
        profile.device_id.clone(),
        profile.account_id.clone(),
        before,
        -i64::from(local_timezone_offset_seconds()),
        page_size,
    )
}

fn weight_profile(profile: &ProfileContext) -> MCPWeightProfile {
    MCPWeightProfile {
        profile_id: profile.profile_id.clone(),
        name: profile.name.clone(),
        scale_name: profile.scale_name.clone(),
        scale_model: profile.scale_model.clone(),
        height_cm: profile.height.parse().ok(),
        weight_target_kg: profile.weight_target.parse().ok(),
        last_weight_update_time_ms: profile.last_weight_update_time,
    }
}

fn weight_measurement(
    profile_id: &str,
    record: WeightDataRecord,
) -> Result<MCPWeightResult, String> {
    let measurement = record.data;
    Ok(MCPWeightResult {
        profile_id: profile_id.to_owned(),
        user_name: measurement.user.name,
        measured_at_seconds: parse_required(&measurement.time, "measurement time")?,
        weight_kg: measurement.weight,
        bmi: measurement.bmi,
        body_fat_percent: measurement.body_fat,
        heart_rate_bpm: parse_optional(measurement.heart_rate.as_deref(), "heart rate")?,
        body_water_percent: measurement.body_water,
        muscle_mass_kg: measurement.muscle_mass,
        skeletal_muscle_mass_kg: measurement.skeletal_muscle_mass,
        bone_mass_kg: measurement.bone_mass,
        visceral_fat: parse_optional(measurement.visceral_fat.as_deref(), "visceral fat")?,
        protein_percent: measurement.protein_percent,
        basal_metabolic_rate_kcal: parse_optional(
            measurement.basal_metabolic_rate.as_deref(),
            "basal metabolic rate",
        )?,
        metabolic_age: parse_optional(measurement.metabolic_age.as_deref(), "metabolic age")?,
        body_score: parse_optional(measurement.body_score.as_deref(), "body score")?,
    })
}

fn parse_required<T>(value: &str, field: &str) -> Result<T, String>
where
    T: FromStr,
{
    value
        .parse()
        .map_err(|_| format!("invalid {field}: {value}"))
}

fn parse_optional<T>(value: Option<&str>, field: &str) -> Result<Option<T>, String>
where
    T: FromStr,
{
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_required(value, field))
        .transpose()
}

fn latest_cache_key(profile_id: &str) -> String {
    format!("latest:{profile_id}")
}

fn history_cache_key(profile_id: &str, before: Option<i64>, page_size: u32) -> String {
    match before {
        Some(before) => format!("history:{profile_id}:{before}:{page_size}"),
        None => format!("history:{profile_id}:latest:{page_size}"),
    }
}

fn unknown_profile_error(profile_id: &str) -> String {
    format!("unknown profile_id: {profile_id}; call get_users to obtain a valid profile ID")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        DEFAULT_PAGE_SIZE, WeightService, history_cache_key, latest_cache_key, validate_page_size,
    };
    use crate::cache::CacheStore;
    use crate::config::XiaomiConfig;
    use crate::session::XiaomiSession;
    use crate::test_support::MemoryCredentialStore;
    use crate::time::current_unix_millis;
    use crate::weights::types::MCPWeightResult;

    fn measurement() -> MCPWeightResult {
        MCPWeightResult {
            profile_id: "profile-1".to_owned(),
            user_name: "Daniel".to_owned(),
            measured_at_seconds: 1_784_184_041,
            weight_kg: 89.1,
            bmi: Some(28.8),
            body_fat_percent: Some(26.1),
            heart_rate_bpm: Some(106),
            body_water_percent: Some(55.1),
            muscle_mass_kg: Some(62.2),
            skeletal_muscle_mass_kg: Some(35.9),
            bone_mass_kg: Some(3.6),
            visceral_fat: Some(10),
            protein_percent: Some(13.9),
            basal_metabolic_rate_kcal: Some(1791),
            metabolic_age: Some(19),
            body_score: Some(81),
        }
    }

    #[test]
    fn page_size_defaults_and_enforces_the_existing_limits() {
        assert_eq!(validate_page_size(None).unwrap(), DEFAULT_PAGE_SIZE);
        assert_eq!(validate_page_size(Some(1)).unwrap(), 1);
        assert_eq!(validate_page_size(Some(100)).unwrap(), 100);
        assert!(validate_page_size(Some(0)).is_err());
        assert!(validate_page_size(Some(101)).is_err());
    }

    #[test]
    fn cache_keys_keep_latest_and_history_pages_distinct() {
        assert_eq!(latest_cache_key("profile"), "latest:profile");
        assert_eq!(
            history_cache_key("profile", None, 20),
            "history:profile:latest:20"
        );
        assert_eq!(
            history_cache_key("profile", Some(123_456), 20),
            "history:profile:123456:20"
        );
    }

    #[tokio::test]
    async fn fresh_cached_weight_is_returned_without_xiaomi_login() {
        let credentials = Arc::new(MemoryCredentialStore::default());
        let session = Arc::new(XiaomiSession::new(XiaomiConfig::default(), credentials));
        session.store_token("user-1:pass-token").await.unwrap();
        let cache = CacheStore::in_memory().await.unwrap();
        let expected = measurement();
        cache
            .save_weights(
                "user-1",
                "latest:profile-1",
                "profile-1",
                std::slice::from_ref(&expected),
                current_unix_millis().unwrap(),
            )
            .await
            .unwrap();
        let service = WeightService::new(cache, Arc::clone(&session));

        assert_eq!(service.latest_weight("profile-1").await.unwrap(), expected);
        assert!(
            !session
                .authenticated()
                .await
                .unwrap()
                .client_is_initialized()
        );
    }
}
