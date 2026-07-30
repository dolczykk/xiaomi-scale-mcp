use crate::base::{Client, Result};
use crate::home::utils::get_xiaomi_home_api_url;
use crate::home::{XIAOMI_HOME_BASE_API, XiaomiHomeResponse};
use serde::{Deserialize, Serialize};

const GET_WEIGHT_ACCOUNTS_PATH: &str = "/eco/scale/account/list";

#[derive(Debug, Serialize)]
pub struct GetWeightAccountsRequest {
    pub uid: String,

    #[serde(rename = "deviceId")]
    pub device_id: String,
}

impl GetWeightAccountsRequest {
    pub fn new(uid: String, device_id: String) -> Self {
        Self { uid, device_id }
    }
}

#[derive(Debug, Deserialize)]
pub struct WeightAccount {
    pub uid: String,

    #[serde(rename = "accountId")]
    pub account_id: String,

    pub name: String,
    pub icon: String,

    #[serde(rename = "type")]
    pub account_type: i32,

    pub sex: String,
    pub height: String,

    #[serde(rename = "weightTarget")]
    pub weight_target: String,

    pub birth: String,

    #[serde(rename = "creationTime")]
    pub creation_time: i64,

    #[serde(rename = "accountCode")]
    pub account_code: i32,

    #[serde(rename = "deviceId")]
    pub device_id: String,

    #[serde(rename = "weightUpdateTime")]
    pub weight_update_time: i64,
}

impl Client {
    pub async fn get_weight_accounts(
        &self,
        request: &GetWeightAccountsRequest,
        model: &str,
    ) -> Result<XiaomiHomeResponse<Vec<WeightAccount>>> {
        let mut headers = self.get_default_headers();
        headers.insert("miot-request-page".to_string(), "none".to_string());
        headers.insert("miot-request-model".to_string(), model.to_string());

        let response: XiaomiHomeResponse<Vec<WeightAccount>> = self
            .home_request(
                get_xiaomi_home_api_url(XIAOMI_HOME_BASE_API, self.region.as_str()).as_str(),
                GET_WEIGHT_ACCOUNTS_PATH,
                request,
                &headers,
            )
            .await?;

        Ok(response)
    }
}
