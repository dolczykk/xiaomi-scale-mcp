use crate::Client;
use crate::home::utils::get_xiaomi_home_api_url;
use crate::home::{XIAOMI_HOME_CORE_BASE_API, XiaomiHomeResponse};
use serde::{Deserialize, Serialize};

const GET_DEVICES_PATH: &str = "/home/device_list_page";

#[derive(Debug, Serialize)]
pub struct GetDevicesRequest {
    pub ssid: String,
    pub bssid: String,

    #[serde(rename = "getVirtualModel")]
    pub get_virtual_model: bool,

    #[serde(rename = "getHuamiDevices")]
    pub get_huami_devices: u8,
    pub get_split_device: bool,
    pub support_smart_home: bool,
    pub get_cariot_device: bool,
    pub get_third_device: bool,
    pub get_phone_device: bool,
    pub get_miwear_device: bool,
}

impl Default for GetDevicesRequest {
    fn default() -> Self {
        GetDevicesRequest {
            ssid: String::from("<unknown ssid>"),
            bssid: String::from("02:00:00:00:00:00"),
            get_virtual_model: true,
            get_huami_devices: 1,
            get_split_device: true,
            support_smart_home: true,
            get_cariot_device: true,
            get_third_device: true,
            get_phone_device: true,
            get_miwear_device: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GetDeviceResponse {
    pub list: Vec<DeviceItem>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceItem {
    #[serde(rename = "did")]
    pub device_id: String,

    #[serde(rename = "uid")]
    pub user_id: u32,

    pub token: String,
    pub name: String,
    pub pid: u32,

    #[serde(rename = "local_ip")]
    pub local_ip: Option<String>,

    pub mac: String,
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub rssi: Option<i32>,
    pub model: String,

    #[serde(rename = "permitLevel")]
    pub permit_level: u8,

    #[serde(rename = "isOnline")]
    pub is_online: bool,
    pub spec_type: Option<String>,
}

impl Client {
    pub async fn get_devices(
        &self,
        request: &GetDevicesRequest,
    ) -> crate::Result<XiaomiHomeResponse<GetDeviceResponse>> {
        let mut headers = self.get_default_headers();
        headers.insert(
            "miot-request-page".to_string(),
            "SmartHomeMainActivity".to_string(),
        );

        let response: XiaomiHomeResponse<GetDeviceResponse> = self
            .home_request(
                get_xiaomi_home_api_url(XIAOMI_HOME_CORE_BASE_API, self.region.as_str()).as_str(),
                GET_DEVICES_PATH,
                &request,
                &headers,
            )
            .await?;

        Ok(response)
    }
}
