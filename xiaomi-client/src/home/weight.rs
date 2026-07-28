use crate::home::utils::get_xiaomi_home_api_url;
use crate::home::{XIAOMI_HOME_BASE_API, XiaomiHomeResponse};
use crate::{Client, Result};
use serde::{Deserialize, Serialize};

const WEIGHT_INDEX_INFO_PATH: &str = "/eco/common/scale/indexInfo";

#[derive(Debug, Serialize)]
pub struct WeightIndexInfoRequest {
    pub uid: String,

    #[serde(rename = "accountId")]
    pub account_id: String,

    #[serde(rename = "deviceId")]
    pub device_id: String,
}

impl WeightIndexInfoRequest {
    pub fn new(uid: String, account_id: String, device_id: String) -> Self {
        Self {
            uid,
            account_id,
            device_id,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WeightIndexInfoResponse {
    pub show: bool,
    pub update: bool,

    #[serde(rename = "twoTerminal")]
    pub two_terminal: bool,

    #[serde(rename = "userDiff")]
    pub user_diff: WeightUserDiff,

    #[serde(rename = "accountInfoList")]
    pub account_info_list: Vec<WeightAccountInfo>,

    #[serde(rename = "deviceConfigInfoList")]
    pub device_config_info_list: Vec<WeightDeviceConfigInfo>,
}

#[derive(Debug, Deserialize)]
pub struct WeightUserDiff {
    pub sex: String,
    pub height: String,
    pub birth: String,

    #[serde(rename = "weightTarget")]
    pub weight_target: String,

    #[serde(rename = "accountId")]
    pub account_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct WeightAccountInfo {
    pub id: i64,

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

    #[serde(rename = "createTime")]
    pub create_time: i64,

    #[serde(rename = "accountCode")]
    pub account_code: i32,

    #[serde(rename = "deviceId")]
    pub device_id: String,

    pub uid: String,

    #[serde(rename = "updateTime")]
    pub update_time: i64,

    #[serde(rename = "weightUpdateTime")]
    pub weight_update_time: i64,

    #[serde(rename = "guideShow")]
    pub guide_show: i32,

    #[serde(rename = "useGuideShow")]
    pub use_guide_show: i32,
}

#[derive(Debug, Deserialize)]
pub struct WeightDeviceConfigInfo {
    pub id: i64,
    pub did: String,
    pub uid: i64,

    #[serde(rename = "accountId")]
    pub account_id: i64,

    #[serde(rename = "onOff")]
    pub on_off: i32,

    pub unit: String,

    #[serde(rename = "infoSync")]
    pub info_sync: i32,

    #[serde(rename = "createTime")]
    pub create_time: i64,

    #[serde(rename = "updateTime")]
    pub update_time: i64,

    #[serde(rename = "isVoicePlay")]
    pub is_voice_play: i32,
}

impl Client {
    pub async fn get_weight_index_info(
        &self,
        request: &WeightIndexInfoRequest,
        model: &str,
    ) -> Result<XiaomiHomeResponse<WeightIndexInfoResponse>> {
        let mut headers = self.get_default_headers();
        headers.insert("miot-request-page".to_string(), "none".to_string());
        headers.insert("miot-request-model".to_string(), model.to_string());

        let response: XiaomiHomeResponse<WeightIndexInfoResponse> = self
            .home_request(
                get_xiaomi_home_api_url(XIAOMI_HOME_BASE_API, self.region.as_str()).as_str(),
                WEIGHT_INDEX_INFO_PATH,
                request,
                &headers,
            )
            .await?;

        Ok(response)
    }
}
