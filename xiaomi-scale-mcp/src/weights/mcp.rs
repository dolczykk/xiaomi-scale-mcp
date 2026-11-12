use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, tool, tool_router};

use crate::time::parse_date_millis;

use super::service::{WeightService, validate_page_size};
use super::types::{
    MCPHistoricalWeightsRequest, MCPHistoricalWeightsResponse, MCPWeightProfilesResponse,
    MCPWeightRequest, MCPWeightResult,
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

    #[tool(
        description = "Get a page of historical weights for a Xiaomi scale profile, optionally filtered from after (inclusive) to before (exclusive)"
    )]
    async fn get_historical_weights(
        &self,
        Parameters(MCPHistoricalWeightsRequest {
            profile_id,
            after,
            before,
            page_size,
        }): Parameters<MCPHistoricalWeightsRequest>,
    ) -> Result<Json<MCPHistoricalWeightsResponse>, String> {
        let after = after
            .as_deref()
            .map(|value| parse_date_millis(value, "after"))
            .transpose()?;

        let before = before
            .as_deref()
            .map(|value| parse_date_millis(value, "before"))
            .transpose()?;

        let page_size = validate_page_size(page_size)?;
        self.weights
            .historical_weights(&profile_id, after, before, page_size)
            .await
            .map(|weights| Json(MCPHistoricalWeightsResponse { weights }))
            .map_err(|error| error.to_string())
    }

    #[tool(description = "Get available Xiaomi scale weight profiles")]
    async fn get_users(&self) -> Result<Json<MCPWeightProfilesResponse>, String> {
        self.weights
            .profiles()
            .await
            .map(|profiles| Json(MCPWeightProfilesResponse { profiles }))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::McpWeightTools;
    use crate::weights::types::MCPHistoricalWeightsRequest;
    use serde_json::json;

    #[test]
    fn history_input_schema_exposes_an_optional_date_string() {
        let tools = McpWeightTools::tool_router().list_all();
        let tool = tools
            .iter()
            .find(|tool| tool.name == "get_historical_weights")
            .unwrap();
        let schema = serde_json::to_value(&tool.input_schema).unwrap();

        for parameter in ["after", "before"] {
            assert_eq!(
                schema["properties"][parameter]["type"],
                json!(["string", "null"])
            );

            assert!(schema["properties"][parameter].get("format").is_none());

            assert!(
                !schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(parameter))
            );
        }
    }

    #[test]
    fn history_request_accepts_date_strings_and_absent_or_null_before() {
        for before in ["2026-11-12", "2026-11-12T15:30:00+01:00"] {
            let request: MCPHistoricalWeightsRequest = serde_json::from_value(json!({
                "profile_id": "profile", "before": before,
            }))
            .unwrap();
            assert_eq!(request.before.as_deref(), Some(before));
        }
        for value in [
            json!({"profile_id": "profile"}),
            json!({"profile_id": "profile", "before": null}),
        ] {
            let request: MCPHistoricalWeightsRequest = serde_json::from_value(value).unwrap();
            assert!(request.before.is_none());
        }
        assert!(
            serde_json::from_value::<MCPHistoricalWeightsRequest>(json!({
                "profile_id": "profile", "before": 1794441600000_i64,
            }))
            .is_err()
        );
    }

    #[test]
    fn history_request_accepts_optional_after_and_rejects_numeric_after() {
        for after in ["2026-11-01", "2026-11-01T15:30:00+01:00"] {
            let request: MCPHistoricalWeightsRequest = serde_json::from_value(json!({
                "profile_id": "profile", "after": after,
            }))
            .unwrap();

            assert_eq!(request.after.as_deref(), Some(after));
        }

        for value in [
            json!({"profile_id": "profile"}),
            json!({"profile_id": "profile", "after": null}),
        ] {
            let request: MCPHistoricalWeightsRequest = serde_json::from_value(value).unwrap();

            assert!(request.after.is_none());
        }

        assert!(
            serde_json::from_value::<MCPHistoricalWeightsRequest>(json!({
                "profile_id": "profile", "after": 1794441600000_i64,
            }))
            .is_err()
        );
    }

    #[test]
    fn router_exposes_the_compatible_weight_tool_contract() {
        let tools = McpWeightTools::tool_router().list_all();
        for name in ["get_users", "get_weight", "get_historical_weights"] {
            let tool = tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("missing tool {name}"));
            assert_eq!(
                tool.output_schema
                    .as_ref()
                    .and_then(|schema| schema.get("type"))
                    .and_then(|value| value.as_str()),
                Some("object"),
                "{name} must expose an object output schema"
            );
        }
    }
}
