use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;
use xiaomi_client::Client;
use xiaomi_client::home::account::{GetWeightAccountsRequest, WeightAccount};
use xiaomi_client::home::devices::{DeviceItem, GetDevicesRequest};
use xiaomi_client::home::weight::{WeightDataRecord, WeightUserDataRequest};
use xiaomi_client::utils::local_timezone_offset_seconds;
use zeroize::Zeroizing;

use crate::config::XiaomiConfig;
use crate::credentials::validate_token;
use crate::dal::{CacheDal, CacheEntry};
use crate::models::{MCPWeightProfile, MCPWeightResult};
use crate::utils::{parse_optional, parse_required, profile_id};

use super::consts::CACHE_TTL_MS;
use super::utils::{history_weight_cache_key, latest_weight_cache_key, now_ms};

impl<T> CacheEntry<T> {
    fn is_fresh(&self, now_ms: i64) -> bool {
        now_ms >= self.fetched_at_ms && now_ms.saturating_sub(self.fetched_at_ms) <= CACHE_TTL_MS
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct WeightProfileContext {
    profile_id: String,
    account_id: String,
    user_id: String,
    device_id: String,
    scale_name: String,
    scale_model: String,
    name: String,
    height: String,
    weight_target: String,
    last_weight_update_time: i64,
}

pub struct WeightRepository {
    cache: CacheDal,
    xiaomi_user_id: String,
    token: Zeroizing<String>,
    xiaomi: XiaomiConfig,
    client: OnceCell<Client>,
}

impl WeightRepository {
    pub fn from_token(
        cache: CacheDal,
        xiaomi: XiaomiConfig,
        token: String,
    ) -> anyhow::Result<Self> {
        let token = Zeroizing::new(token);
        let token = token.trim();
        validate_token(token)?;

        let (xiaomi_user_id, pass_token) = token
            .split_once(':')
            .context("validated Xiaomi token is missing a separator")?;
        debug_assert!(!pass_token.is_empty());

        Ok(Self {
            cache,
            xiaomi_user_id: xiaomi_user_id.to_string(),
            token: Zeroizing::new(token.to_string()),
            xiaomi,
            client: OnceCell::new(),
        })
    }

    pub async fn get_profiles(&self) -> anyhow::Result<Vec<MCPWeightProfile>> {
        let profiles = self.profile_contexts().await?;

        Ok(profiles.iter().map(weight_profile).collect())
    }

    pub async fn get_latest_weight(&self, profile_id: &str) -> anyhow::Result<MCPWeightResult> {
        let cache_key = latest_weight_cache_key(profile_id);
        let measurements = self
            .weight_page(profile_id, None, 1, &cache_key, false)
            .await?;

        measurements
            .into_iter()
            .next()
            .context("no weight measurements found for profile")
    }

    pub async fn get_historical_weights(
        &self,
        profile_id: &str,
        before: Option<i64>,
        page_size: u32,
    ) -> anyhow::Result<Vec<MCPWeightResult>> {
        let cache_key = history_weight_cache_key(profile_id, before, page_size);
        self.weight_page(profile_id, before, page_size, &cache_key, true)
            .await
    }

    async fn profile_contexts(&self) -> anyhow::Result<Vec<WeightProfileContext>> {
        let now_ms = now_ms()?;
        let cached = self
            .cache
            .load_profiles::<Vec<WeightProfileContext>>(&self.xiaomi_user_id)
            .await
            .unwrap_or_else(|error| {
                log::warn!("Failed to read Xiaomi profile cache: {error:#}");
                None
            });

        if let Some(entry) = cached.as_ref().filter(|entry| entry.is_fresh(now_ms)) {
            return Ok(entry.value.clone());
        }

        let profiles = match self.fetch_profiles().await {
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
            .save_profiles(&self.xiaomi_user_id, &profiles, now_ms)
            .await
        {
            log::warn!("Failed to update Xiaomi profile cache: {error:#}");
        }

        Ok(profiles)
    }

    async fn weight_page(
        &self,
        profile_id: &str,
        before: Option<i64>,
        page_size: u32,
        cache_key: &str,
        empty_page_is_valid: bool,
    ) -> anyhow::Result<Vec<MCPWeightResult>> {
        let now_ms = now_ms()?;
        let cached = self
            .cache
            .load_weights::<Vec<MCPWeightResult>>(&self.xiaomi_user_id, cache_key)
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

        let before_ms = before.unwrap_or(now_ms);
        let measurements = match self
            .fetch_weight_page(profile_id, before_ms, page_size)
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
                &self.xiaomi_user_id,
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

    async fn fetch_profiles(&self) -> anyhow::Result<Vec<WeightProfileContext>> {
        let client = self.client().await?;
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
                    .map(|account| map_weight_profile(&scale, account)),
            );
        }

        log::info!("Discovered {} Xiaomi weight profiles", profiles.len());
        Ok(profiles)
    }

    async fn fetch_weight_page(
        &self,
        profile_id: &str,
        before_ms: i64,
        page_size: u32,
    ) -> anyhow::Result<Vec<MCPWeightResult>> {
        let profiles = self.profile_contexts().await?;
        let profile = profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
            .with_context(|| unknown_profile_error(profile_id))?;
        let request = weight_data_request(profile, before_ms, page_size);
        let response = self
            .client()
            .await?
            .get_weight_user_data(&request)
            .await
            .context("failed to fetch Xiaomi weight measurements")?;

        response
            .result
            .into_iter()
            .map(|record| weight_result(profile_id, record).map_err(anyhow::Error::msg))
            .collect()
    }

    async fn client(&self) -> anyhow::Result<&Client> {
        self.client
            .get_or_try_init(|| async {
                let mut client = self.xiaomi.client()?;

                log::info!("Authenticating with Xiaomi token");
                client
                    .login_with_token(&self.token)
                    .await
                    .context("Xiaomi token authentication failed")?;

                Ok(client)
            })
            .await
    }
}

fn map_weight_profile(scale: &DeviceItem, account: WeightAccount) -> WeightProfileContext {
    WeightProfileContext {
        profile_id: profile_id(&scale.device_id, &account.account_id),
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
    profile: &WeightProfileContext,
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

fn weight_profile(profile: &WeightProfileContext) -> MCPWeightProfile {
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

fn weight_result(profile_id: &str, record: WeightDataRecord) -> Result<MCPWeightResult, String> {
    let measurement = record.data;

    Ok(MCPWeightResult {
        profile_id: profile_id.to_string(),
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

fn unknown_profile_error(profile_id: &str) -> String {
    format!("unknown profile_id: {profile_id}; call get_users to obtain a valid profile ID")
}

#[cfg(test)]
mod tests {
    use super::{CACHE_TTL_MS, CacheEntry, WeightRepository, now_ms, weight_result};
    use crate::config::XiaomiConfig;
    use crate::dal::CacheDal;
    use crate::models::MCPWeightResult;
    use xiaomi_client::home::weight::{WeightDataRecord, WeightMeasurement, WeightMeasurementUser};

    fn repository(cache: CacheDal, user_id: &str) -> WeightRepository {
        WeightRepository::from_token(
            cache,
            XiaomiConfig::default(),
            format!("{user_id}:pass-token"),
        )
        .unwrap()
    }

    fn measurement() -> MCPWeightResult {
        MCPWeightResult {
            profile_id: "blt.4.scale:account-1".to_string(),
            user_name: "Daniel".to_string(),
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
    fn cache_entry_is_fresh_only_inside_ttl() {
        let entry = CacheEntry {
            fetched_at_ms: 10_000,
            value: (),
        };

        assert!(entry.is_fresh(10_000 + CACHE_TTL_MS));
        assert!(!entry.is_fresh(10_000 + CACHE_TTL_MS + 1));
        assert!(!entry.is_fresh(9_999));
    }

    #[tokio::test]
    async fn stale_records_remain_available_until_retention_expires() {
        let cache = CacheDal::in_memory().await.unwrap();
        let repository = repository(cache, "xiaomi-user-1");
        let fetched_at_ms = 1_000_000;

        repository
            .cache
            .save_weights(
                "xiaomi-user-1",
                "latest:profile",
                "profile",
                &[measurement()],
                fetched_at_ms,
            )
            .await
            .unwrap();
        let cached = repository
            .cache
            .load_weights::<Vec<MCPWeightResult>>("xiaomi-user-1", "latest:profile")
            .await
            .unwrap()
            .unwrap();

        assert!(!cached.is_fresh(fetched_at_ms + CACHE_TTL_MS + 1));
        assert_eq!(cached.value, vec![measurement()]);
    }

    #[tokio::test]
    async fn fresh_weight_cache_is_returned_without_authentication() {
        let cache = CacheDal::in_memory().await.unwrap();
        let repository = repository(cache, "xiaomi-user-1");
        let measurement = measurement();

        repository
            .cache
            .save_weights(
                "xiaomi-user-1",
                "latest:blt.4.scale:account-1",
                "blt.4.scale:account-1",
                std::slice::from_ref(&measurement),
                now_ms().unwrap(),
            )
            .await
            .unwrap();

        let result = repository
            .get_latest_weight("blt.4.scale:account-1")
            .await
            .unwrap();

        assert_eq!(result, measurement);
        assert!(repository.client.get().is_none());
    }

    #[test]
    fn weight_record_is_converted_to_curated_output() {
        let record = WeightDataRecord {
            data: WeightMeasurement {
                index: "1".to_string(),
                miid: String::new(),
                duid: "1".to_string(),
                user_type: "1".to_string(),
                weight: 89.1,
                heart_rate: Some("106".to_string()),
                status: "0".to_string(),
                time: "1784184041".to_string(),
                body_fat: Some(26.1),
                muscle_mass: Some(62.2),
                body_water: Some(55.1),
                bone_mass: Some(3.6),
                visceral_fat: Some("10".to_string()),
                protein_percent: Some(13.9),
                skeletal_muscle_mass: Some(35.9),
                bmi: Some(28.8),
                ideal_weight: Some(67.2),
                muscle_correction: Some(-8.7),
                weight_correction: Some(-21.9),
                fat_correction: Some(-13.2),
                waist_hip_ratio: Some(0.8),
                basal_metabolic_rate: Some("1791".to_string()),
                body_type: Some("4".to_string()),
                metabolic_age: Some("19".to_string()),
                body_score: Some("81".to_string()),
                muscle_percent: Some(69.8),
                bone_mass_percentage: Some(4.0),
                fat_mass: Some(23.3),
                lean_body_mass: Some(65.8),
                body_water_mass: Some(49.1),
                protein_mass: Some(12.4),
                body_res: Some(387.4),
                body_res2: Some(347.9),
                report_from: None,
                user: WeightMeasurementUser {
                    name: "Daniel".to_string(),
                    uid: "1756972511".to_string(),
                    account_id: "1756972511".to_string(),
                    account_type: 1,
                    sex: "1".to_string(),
                    height: "176".to_string(),
                    weight_target: "89.1".to_string(),
                    birth: "1061676000000".to_string(),
                    account_code: 1,
                    icon: String::new(),
                },
            },
            data_version: 1,
            uid: 1_756_972_511,
            account_id: 1_756_972_511,
            create_time: 1_784_184_041_000,
            model: String::new(),
            sn: "scale-serial".to_string(),
            device_id: "blt.4.scale".to_string(),
            from_source: 2,
        };

        let result = weight_result("blt.4.scale:1756972511", record).unwrap();

        assert_eq!(result.profile_id, "blt.4.scale:1756972511");
        assert_eq!(result.user_name, "Daniel");
        assert_eq!(result.measured_at_seconds, 1_784_184_041);
        assert_eq!(result.weight_kg, 89.1);
        assert_eq!(result.heart_rate_bpm, Some(106));
        assert_eq!(result.basal_metabolic_rate_kcal, Some(1791));
    }
}
