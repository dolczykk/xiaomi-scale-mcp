use std::collections::HashMap;
use std::time::Duration;

use crate::{
    auth::PendingAuth,
    errors::XiaomiError,
    utils::{
        is_daylight_saving_time, local_timezone_name, local_timezone_offset,
        random_lowercase_string,
    },
};

pub const APP_XIAOMI_HOME: &str = "xiaomiio";
pub type Result<T> = std::result::Result<T, XiaomiError>;

#[derive(Clone)]
pub struct Client {
    pub(crate) client: reqwest::Client,
    pub(crate) sid: String,
    pub(crate) pass_token: String,
    pub(crate) c_user_id: String,
    pub(crate) service_token: String,
    pub(crate) ssecurity: Vec<u8>,
    pub(crate) user_id: i64,
    pub(crate) auth: Option<PendingAuth>,
    pub(crate) device_id: Option<String>,
    pub(crate) locale: String,
    pub(crate) region: String,
}

impl Client {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .cookie_store(true)
                .build()?,
            sid: APP_XIAOMI_HOME.to_string(),
            pass_token: String::new(),
            c_user_id: String::new(),
            service_token: String::new(),
            ssecurity: Vec::new(),
            user_id: 0,
            auth: None,
            device_id: Some(random_lowercase_string(6)),
            locale: String::from("en_US"),
            region: String::from("cn"),
        })
    }

    #[must_use]
    pub fn with_sid(mut self, sid: impl Into<String>) -> Self {
        self.sid = sid.into();

        self
    }

    #[must_use]
    pub fn with_device_id(mut self, device_id: String) -> Self {
        self.device_id = Some(device_id);

        self
    }

    #[must_use]
    pub fn with_locale(mut self, locale: String) -> Self {
        self.locale = locale;

        self
    }

    #[must_use]
    pub fn with_region(mut self, region: String) -> Self {
        self.region = region;

        self
    }

    #[must_use]
    pub fn device_id(&self) -> Option<String> {
        self.device_id.clone()
    }

    #[must_use]
    pub fn token(&self) -> String {
        format!("{}:{}", self.user_id, self.pass_token)
    }

    pub(crate) fn get_default_cookies(&self) -> String {
        let timezone_offset = local_timezone_offset();
        let is_daylight = is_daylight_saving_time();
        let dst_offset = if is_daylight { 60 * 60 * 1000 } else { 0 };
        let country_code = self.locale.split_once('_').map_or("CN", |(_, country)| {
            if country.is_empty() { "CN" } else { country }
        });
        let device_id = self.device_id.as_deref().unwrap_or_default();
        let timezone_name = local_timezone_name();
        let is_daylight = i32::from(is_daylight).to_string();
        let dst_offset = dst_offset.to_string();

        let mut cookies = String::new();
        cookies.push_str("userId=");
        cookies.push_str(&self.user_id.to_string());
        cookies.push_str(";cUserId=");
        cookies.push_str(&self.c_user_id);
        cookies.push_str(";yetAnotherServiceToken=");
        cookies.push_str(&self.service_token);
        cookies.push_str(";serviceToken=");
        cookies.push_str(&self.service_token);
        cookies.push_str(";timezone_id=");
        cookies.push_str(&timezone_name);
        cookies.push_str(";timezone=GMT");
        cookies.push_str(&timezone_offset);
        cookies.push_str(";is_daylight=");
        cookies.push_str(&is_daylight);
        cookies.push_str(";dst_offset=");
        cookies.push_str(&dst_offset);
        cookies.push_str(";channel=MI_APP_STORE");
        cookies.push_str(";countryCode=");
        cookies.push_str(country_code);
        cookies.push_str(";PassportDeviceId=");
        cookies.push_str(device_id);
        cookies.push_str(";locale=");
        cookies.push_str(&self.locale);

        cookies
    }

    pub(crate) fn default_headers() -> HashMap<String, String> {
        let mut headers = HashMap::new();

        headers.insert(
            "miot-encrypt-algorithm".to_string(),
            "ENCRYPT-RC4".to_string(),
        );
        headers.insert("accept-encoding".to_string(), "identity".to_string());
        headers.insert("miot-accept-encoding".to_string(), "GZIP".to_string());
        headers.insert("origin-from".to_string(), "MiHome".to_string());
        headers.insert(
            "origin-model".to_string(),
            "mphone.phone.online".to_string(),
        );
        headers.insert(
            "miot-origin-request-version".to_string(),
            "android;phone;11.6.625.4316".to_string(),
        );
        headers.insert(
            "x-xiaomi-protocal-flag-cli".to_string(),
            "PROTOCAL-HTTP2".to_string(),
        );

        headers
    }
}
