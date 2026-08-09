use std::sync::Arc;

use crate::models::{
    MCPHistoricalWeightsRequest, MCPWeightProfile, MCPWeightRequest, MCPWeightResult,
};
use crate::state::State;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

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
        let repository = self
            .state
            .repository()
            .await
            .map_err(|error| error.to_string())?;
        let result = repository
            .get_latest_weight(&profile_id)
            .await
            .map_err(|error| error.to_string())?;

        Ok(Json(result))
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
        if before.is_some_and(|timestamp| timestamp <= 0) {
            return Err("before must be a positive Unix timestamp in milliseconds".to_string());
        }

        let page_size = validate_page_size(page_size)?;
        let repository = self
            .state
            .repository()
            .await
            .map_err(|error| error.to_string())?;
        let results = repository
            .get_historical_weights(&profile_id, before, page_size)
            .await
            .map_err(|error| error.to_string())?;

        Ok(Json(results))
    }

    #[tool(description = "Get available Xiaomi scale weight profiles")]
    async fn get_users(&self) -> Result<Json<Vec<MCPWeightProfile>>, String> {
        let repository = self
            .state
            .repository()
            .await
            .map_err(|error| error.to_string())?;
        let profiles = repository
            .get_profiles()
            .await
            .map_err(|error| error.to_string())?;

        Ok(Json(profiles))
    }
}

fn validate_page_size(page_size: Option<u32>) -> Result<u32, String> {
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
        return Err("page_size must be between 1 and 100".to_string());
    }

    Ok(page_size)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PAGE_SIZE, Weight, validate_page_size};

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
