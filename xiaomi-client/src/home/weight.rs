use crate::home::utils::get_xiaomi_home_api_url;
use crate::home::{XIAOMI_HOME_BASE_API, XiaomiHomeResponse};
use crate::{Client, Result};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};

const WEIGHT_INDEX_INFO_PATH: &str = "/eco/common/scale/indexInfo";
const GET_WEIGHT_DATA_PATH: &str = "/eco/common/scale/getUserDataByPage";

#[derive(Debug, Serialize)]
pub struct WeightUserDataRequest {
    pub model: String,
    pub uid: String,

    #[serde(rename = "did")]
    pub device_id: String,

    #[serde(rename = "accountId")]
    pub account_id: String,

    #[serde(rename = "beginTime")]
    pub begin_time: i64,

    #[serde(rename = "endTime")]
    pub end_time: i64,

    #[serde(rename = "pageSize")]
    pub page_size: u32,
}

impl WeightUserDataRequest {
    pub fn new(
        model: String,
        uid: String,
        device_id: String,
        account_id: String,
        begin_time: i64,
        end_time: i64,
        page_size: u32,
    ) -> Self {
        Self {
            model,
            uid,
            device_id,
            account_id,
            begin_time,
            end_time,
            page_size,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WeightDataRecord {
    #[serde(deserialize_with = "deserialize_weight_measurement")]
    pub data: WeightMeasurement,

    #[serde(rename = "dataVersion")]
    pub data_version: i32,

    pub uid: i64,

    #[serde(rename = "accountId")]
    pub account_id: i64,

    #[serde(rename = "createTime")]
    pub create_time: i64,

    pub model: String,
    pub sn: String,

    #[serde(rename = "did")]
    pub device_id: String,

    #[serde(rename = "fromSource")]
    pub from_source: i32,
}

#[derive(Debug, Deserialize)]
pub struct WeightMeasurement {
    pub idx: String,
    pub miid: String,
    pub duid: String,

    #[serde(rename = "userType")]
    pub user_type: String,

    pub weight: f64,

    #[serde(rename = "heartRate")]
    pub heart_rate: Option<String>,

    pub status: String,
    pub time: String,
    pub bfp: Option<f64>,
    pub slm: Option<f64>,
    pub bwp: Option<f64>,
    pub bmc: Option<f64>,
    pub vfl: Option<String>,
    pub pp: Option<f64>,
    pub smm: Option<f64>,
    pub bmi: Option<f64>,
    pub swt: Option<f64>,
    pub mc: Option<f64>,
    pub wc: Option<f64>,
    pub fc: Option<f64>,
    pub whr: Option<f64>,
    pub bmr: Option<String>,
    pub bt: Option<String>,
    pub ma: Option<String>,
    pub sbc: Option<String>,
    pub slp: Option<f64>,
    pub bmcp: Option<f64>,
    pub bfm: Option<f64>,
    pub ffm: Option<f64>,
    pub bwm: Option<f64>,
    pub pm: Option<f64>,

    #[serde(rename = "bodyRes")]
    pub body_res: Option<f64>,

    #[serde(rename = "bodyRes2")]
    pub body_res2: Option<f64>,

    pub user: WeightMeasurementUser,
}

#[derive(Debug, Deserialize)]
pub struct WeightMeasurementUser {
    pub name: String,
    pub uid: String,

    #[serde(rename = "accountId")]
    pub account_id: String,

    #[serde(rename = "type")]
    pub account_type: i32,

    pub sex: String,
    pub height: String,

    #[serde(rename = "weightTarget")]
    pub weight_target: String,

    pub birth: String,

    #[serde(rename = "accountCode")]
    pub account_code: i32,

    pub icon: String,
}

fn deserialize_weight_measurement<'de, D>(
    deserializer: D,
) -> std::result::Result<WeightMeasurement, D::Error>
where
    D: Deserializer<'de>,
{
    let data = String::deserialize(deserializer)?;
    serde_json::from_str(&data).map_err(D::Error::custom)
}

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
    pub async fn get_weight_user_data(
        &self,
        request: &WeightUserDataRequest,
    ) -> Result<XiaomiHomeResponse<Vec<WeightDataRecord>>> {
        let mut headers = self.get_default_headers();
        headers.insert("miot-request-page".to_string(), "none".to_string());
        headers.insert("miot-request-model".to_string(), request.model.clone());

        let response: XiaomiHomeResponse<Vec<WeightDataRecord>> = self
            .home_request(
                get_xiaomi_home_api_url(XIAOMI_HOME_BASE_API, self.region.as_str()).as_str(),
                GET_WEIGHT_DATA_PATH,
                request,
                &headers,
            )
            .await?;

        Ok(response)
    }

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
