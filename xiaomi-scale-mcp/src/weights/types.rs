use rmcp::schemars;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MCPWeightRequest {
    #[schemars(description = "Profile ID returned by the get_users tool")]
    pub(crate) profile_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MCPHistoricalWeightsRequest {
    #[schemars(description = "Profile ID returned by the get_users tool")]
    pub(crate) profile_id: String,

    #[schemars(description = "Unix timestamp in milliseconds to fetch measurements before")]
    pub(crate) before: Option<i64>,

    #[schemars(description = "Number of measurements to return, from 1 through 100")]
    pub(crate) page_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MCPWeightProfile {
    pub(crate) profile_id: String,
    pub(crate) name: String,
    pub(crate) scale_name: String,
    pub(crate) scale_model: String,
    pub(crate) height_cm: Option<f64>,
    pub(crate) weight_target_kg: Option<f64>,
    pub(crate) last_weight_update_time_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MCPWeightResult {
    pub(crate) profile_id: String,
    pub(crate) user_name: String,
    pub(crate) measured_at_seconds: i64,
    pub(crate) weight_kg: f64,
    pub(crate) bmi: Option<f64>,
    pub(crate) body_fat_percent: Option<f64>,
    pub(crate) heart_rate_bpm: Option<u32>,
    pub(crate) body_water_percent: Option<f64>,
    pub(crate) muscle_mass_kg: Option<f64>,
    pub(crate) skeletal_muscle_mass_kg: Option<f64>,
    pub(crate) bone_mass_kg: Option<f64>,
    pub(crate) visceral_fat: Option<u32>,
    pub(crate) protein_percent: Option<f64>,
    pub(crate) basal_metabolic_rate_kcal: Option<u32>,
    pub(crate) metabolic_age: Option<u32>,
    pub(crate) body_score: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MCPWeightProfilesResponse {
    pub(crate) profiles: Vec<MCPWeightProfile>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub(crate) struct MCPHistoricalWeightsResponse {
    pub(crate) weights: Vec<MCPWeightResult>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(super) struct ProfileContext {
    pub(super) profile_id: String,
    pub(super) account_id: String,
    pub(super) user_id: String,
    pub(super) device_id: String,
    pub(super) scale_name: String,
    pub(super) scale_model: String,
    pub(super) name: String,
    pub(super) height: String,
    pub(super) weight_target: String,
    pub(super) last_weight_update_time: i64,
}
