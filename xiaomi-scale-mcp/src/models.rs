use rmcp::schemars;

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct MCPWeightRequest {
    #[schemars(description = "Profile ID returned by the get_users tool")]
    pub profile_id: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct MCPHistoricalWeightsRequest {
    #[schemars(description = "Profile ID returned by the get_users tool")]
    pub profile_id: String,

    #[schemars(description = "Unix timestamp in milliseconds to fetch measurements before")]
    pub before: Option<i64>,

    #[schemars(description = "Number of measurements to return, from 1 through 100")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct MCPWeightProfile {
    pub profile_id: String,
    pub name: String,
    pub scale_name: String,
    pub scale_model: String,
    pub height_cm: Option<f64>,
    pub weight_target_kg: Option<f64>,
    pub last_weight_update_time_ms: i64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, schemars::JsonSchema)]
pub struct MCPWeightResult {
    pub profile_id: String,
    pub user_name: String,
    pub measured_at_seconds: i64,
    pub weight_kg: f64,
    pub bmi: Option<f64>,
    pub body_fat_percent: Option<f64>,
    pub heart_rate_bpm: Option<u32>,
    pub body_water_percent: Option<f64>,
    pub muscle_mass_kg: Option<f64>,
    pub skeletal_muscle_mass_kg: Option<f64>,
    pub bone_mass_kg: Option<f64>,
    pub visceral_fat: Option<u32>,
    pub protein_percent: Option<f64>,
    pub basal_metabolic_rate_kcal: Option<u32>,
    pub metabolic_age: Option<u32>,
    pub body_score: Option<u32>,
}
