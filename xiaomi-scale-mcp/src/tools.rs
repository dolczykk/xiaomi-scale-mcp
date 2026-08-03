use std::sync::Arc;

use crate::models::{
    MCPHistoricalWeightsRequest, MCPWeightProfile, MCPWeightRequest, MCPWeightResult,
};
use crate::state::{State, WeightProfileContext};
use crate::utils::{current_unix_millis, parse_optional, parse_required};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};
use xiaomi_client::home::weight::{WeightDataRecord, WeightUserDataRequest};
use xiaomi_client::utils::local_timezone_offset_seconds;

const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(Clone)]
pub struct Weight {
    state: Arc<State>,
}

impl Weight {
    pub fn new(state: Arc<State>) -> Self {
        Self { state }
    }
}

#[tool_router(server_handler)]
impl Weight {
    #[tool(description = "Get the latest weight measurement for a Xiaomi scale profile")]
    async fn get_weight(
        &self,
        Parameters(MCPWeightRequest { profile_id }): Parameters<MCPWeightRequest>,
    ) -> Result<Json<MCPWeightResult>, String> {
        let authenticated = self
            .state
            .authenticated()
            .await
            .map_err(|error| error.to_string())?;
        let profile = authenticated
            .profile(&profile_id)
            .ok_or_else(|| unknown_profile_error(&profile_id))?;
        let request = weight_data_request(profile, current_unix_millis()?, 1);
        let response = authenticated
            .client
            .get_weight_user_data(&request)
            .await
            .map_err(|error| error.to_string())?;
        let record = response
            .result
            .into_iter()
            .next()
            .ok_or_else(|| "no weight measurements found for profile".to_string())?;

        Ok(Json(weight_result(&profile_id, record)?))
    }

    #[tool(description = "Get a page of historical weights for a Xiaomi scale profile")]
    async fn get_historical_weights(
        &self,
        Parameters(MCPHistoricalWeightsRequest {
            profile_id,
            before,
            page_size,
        }): Parameters<MCPHistoricalWeightsRequest>,
    ) -> Result<Json<Vec<MCPWeightResult>>, String> {
        let before = before.unwrap_or(current_unix_millis()?);
        if before <= 0 {
            return Err("before must be a positive Unix timestamp in milliseconds".to_string());
        }

        let page_size = validate_page_size(page_size)?;
        let authenticated = self
            .state
            .authenticated()
            .await
            .map_err(|error| error.to_string())?;
        let profile = authenticated
            .profile(&profile_id)
            .ok_or_else(|| unknown_profile_error(&profile_id))?;
        let request = weight_data_request(profile, before, page_size);
        let response = authenticated
            .client
            .get_weight_user_data(&request)
            .await
            .map_err(|error| error.to_string())?;
        let measurements = response
            .result
            .into_iter()
            .map(|record| weight_result(&profile_id, record))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Json(measurements))
    }

    #[tool(description = "Get available Xiaomi scale weight profiles")]
    async fn get_users(&self) -> Result<Json<Vec<MCPWeightProfile>>, String> {
        let authenticated = self
            .state
            .authenticated()
            .await
            .map_err(|error| error.to_string())?;
        let profiles = authenticated.profiles.iter().map(weight_profile).collect();

        Ok(Json(profiles))
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

fn validate_page_size(page_size: Option<u32>) -> Result<u32, String> {
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
        return Err("page_size must be between 1 and 100".to_string());
    }

    Ok(page_size)
}

fn unknown_profile_error(profile_id: &str) -> String {
    let mut message = String::new();
    message.push_str("unknown profile_id: ");
    message.push_str(profile_id);
    message.push_str("; call get_users to obtain a valid profile ID");

    message
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PAGE_SIZE, Weight, validate_page_size, weight_result};
    use xiaomi_client::home::weight::{WeightDataRecord, WeightMeasurement, WeightMeasurementUser};

    #[test]
    fn history_page_size_defaults_to_twenty() {
        assert_eq!(validate_page_size(None).unwrap(), DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn history_page_size_accepts_boundaries() {
        assert_eq!(validate_page_size(Some(1)).unwrap(), 1);
        assert_eq!(validate_page_size(Some(100)).unwrap(), 100);
    }

    #[test]
    fn history_page_size_rejects_values_outside_boundaries() {
        assert!(validate_page_size(Some(0)).is_err());
        assert!(validate_page_size(Some(101)).is_err());
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

    #[test]
    fn router_exposes_weight_tools_with_output_schemas() {
        let tools = Weight::tool_router().list_all();

        for name in ["get_users", "get_weight", "get_historical_weights"] {
            let tool = tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("missing tool {name}"));
            assert!(tool.output_schema.is_some(), "missing schema for {name}");
        }
    }
}
