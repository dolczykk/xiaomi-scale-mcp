use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use super::service::{WeightService, validate_page_size};
use super::types::{
    MCPHistoricalWeightsRequest, MCPWeightProfile, MCPWeightRequest, MCPWeightResult,
};

#[derive(Clone)]
pub(crate) struct McpWeightTools {
    weights: WeightService,
}

impl McpWeightTools {
    pub(crate) fn new(weights: WeightService) -> Self {
        Self { weights }
    }
}

#[tool_router(server_handler)]
impl McpWeightTools {
    #[tool(description = "Get the latest weight measurement for a Xiaomi scale profile")]
    async fn get_weight(
        &self,
        Parameters(MCPWeightRequest { profile_id }): Parameters<MCPWeightRequest>,
    ) -> Result<Json<MCPWeightResult>, String> {
        self.weights
            .latest_weight(&profile_id)
            .await
            .map(Json)
            .map_err(|error| error.to_string())
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
        self.weights
            .historical_weights(&profile_id, before, page_size)
            .await
            .map(Json)
            .map_err(|error| error.to_string())
    }

    #[tool(description = "Get available Xiaomi scale weight profiles")]
    async fn get_users(&self) -> Result<Json<Vec<MCPWeightProfile>>, String> {
        self.weights
            .profiles()
            .await
            .map(Json)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::McpWeightTools;

    #[test]
    fn router_exposes_the_compatible_weight_tool_contract() {
        let tools = McpWeightTools::tool_router().list_all();
        for name in ["get_users", "get_weight", "get_historical_weights"] {
            let tool = tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("missing tool {name}"));
            assert!(tool.output_schema.is_some(), "missing schema for {name}");
        }
    }
}
