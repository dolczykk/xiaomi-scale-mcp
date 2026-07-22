use std::collections::HashMap;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{
    auth::PendingAuth,
    encryption::{decrypt_response_payload, generate_encrypted_params},
    errors::XiaomiError,
    utils::{
        encode_form, is_daylight_saving_time, local_timezone_name,
        local_timezone_offset, normalize_api_signature_uri, random_lowercase_string,
    },
};

pub mod auth;
mod cookies;
pub mod encryption;
pub mod errors;
pub mod home;
mod login;
pub mod utils;

pub const APP_XIAOMI_HOME: &str = "xiaomiio";
pub const APP_MI_FITNESS: &str = "miothealth";

pub(crate) const LOGIN_PREFIX: &str = "&&&START&&&";
pub(crate) const SERVICE_LOGIN_URL: &str = "https://account.xiaomi.com/pass/serviceLogin";
pub(crate) const SERVICE_LOGIN_AUTH2_URL: &str =
    "https://account.xiaomi.com/pass/serviceLoginAuth2";
pub(crate) const OAUTH2_AUTHORIZE_URL: &str = "https://account.xiaomi.com/oauth2/authorize";

pub type Result<T> = std::result::Result<T, XiaomiError>;

#[derive(Debug, Clone)]
pub struct Client {
    client: reqwest::Client,
    sid: String,
    pass_token: String,
    c_user_id: String,
    service_token: String,
    ssecurity: Vec<u8>,
    user_id: i64,
    auth: Option<PendingAuth>,
    device_id: Option<String>,
    locale: String,
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
        })
    }

    pub fn with_sid(mut self, sid: impl Into<String>) -> Self {
        self.sid = sid.into();

        self
    }

    pub fn with_device_id(mut self, device_id: String) -> Self {
        self.device_id = Some(device_id);

        self
    }

    pub fn device_id(&self) -> Option<String> {
        self.device_id.clone()
    }

    pub fn token(&self) -> String {
        format!("{}:{}", self.user_id, self.pass_token)
    }

    fn get_default_cookies(&self) -> String {
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

    fn get_default_headers(&self) -> HashMap<String, String> {
        let mut headers: HashMap<String, String> = HashMap::new();

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

    pub async fn request<T: DeserializeOwned>(
        &self,
        base_url: &str,
        api_url: &str,
        params: &str,
        headers: &HashMap<String, String>,
    ) -> Result<XiaomiHomeResponse<T>> {
        let ssecurity64 = STANDARD.encode(&self.ssecurity);
        let signature_uri = normalize_api_signature_uri(base_url, api_url)?;
        let encrypted_params = generate_encrypted_params(
            &signature_uri,
            "POST",
            &ssecurity64,
            vec![("data".to_string(), params.to_string())],
        )?;
        let nonce64 = encrypted_params
            .iter()
            .find_map(|(key, value)| (key == "_nonce").then_some(value.as_str()))
            .ok_or_else(|| XiaomiError::Auth("encrypted params missing _nonce".to_string()))?
            .to_string();
        let body = encode_form(&encrypted_params);

        let mut request = self
            .client
            .post(format!("{}{}", base_url, api_url))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(COOKIE, self.get_default_cookies())
            .body(body);

        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|err| format!("failed to read error body: {err}"));

            let error = XiaomiError::HttpStatus(format!("{status}: {body}"));

            return Err(error);
        }

        let body = response.bytes().await?;
        let body = std::str::from_utf8(body.as_ref())
            .map_err(|err| XiaomiError::Auth(format!("response body is not UTF-8: {err}")))?;
        let plaintext = decrypt_response_payload(&ssecurity64, &nonce64, body)?;

        let response: XiaomiHomeResponse<T> = XiaomiHomeResponse::from_json(plaintext.as_str())?;
        if response.code != 0 {
            let error = XiaomiError::Api(response.message);

            return Err(error);
        }

        Ok(response)
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound = "T: DeserializeOwned")]
pub struct XiaomiHomeResponse<T: DeserializeOwned> {
    pub code: i32,
    pub message: String,
    pub result: Box<T>,
}

impl<T: DeserializeOwned> XiaomiHomeResponse<T> {
    fn from_json(json: &str) -> Result<Self> {
        let response: XiaomiHomeResponse<T> = serde_json::from_str(json)?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::strip_login_prefix;

    #[test]
    fn strips_login_prefix() {
        let body = br#"&&&START&&&{"sid":"miothealth"}"#;
        assert_eq!(
            strip_login_prefix(body).unwrap(),
            br#"{"sid":"miothealth"}"#.to_vec()
        );
    }

    #[test]
    fn rejects_missing_login_prefix() {
        assert!(matches!(
            strip_login_prefix(br#"{"sid":"miothealth"}"#),
            Err(XiaomiError::WrongLoginPrefix)
        ));
    }
}
