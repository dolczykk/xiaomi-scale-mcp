use std::collections::HashMap;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::header::CONTENT_TYPE;
use serde_json::value::RawValue;

use crate::{
    auth::PendingAuth,
    encryption::{decrypt_response_payload, generate_encrypted_params},
    errors::XiaomiError,
    utils::encode_form,
};

pub mod auth;
mod cookies;
pub mod encryption;
pub mod errors;
mod login;
pub mod utils;

pub const APP_XIAOMI_HOME: &str = "xiaomiio";

pub(crate) const LOGIN_PREFIX: &str = "&&&START&&&";
pub(crate) const SERVICE_LOGIN_URL: &str = "https://account.xiaomi.com/pass/serviceLogin";
pub(crate) const SERVICE_LOGIN_AUTH2_URL: &str =
    "https://account.xiaomi.com/pass/serviceLoginAuth2";
pub(crate) const OAUTH2_AUTHORIZE_URL: &str = "https://account.xiaomi.com/oauth2/authorize";

pub type Result<T> = std::result::Result<T, XiaomiError>;

#[derive(Debug, Clone)]
pub struct Client {
    client: reqwest::Client,
    pass_token: String,
    ssecurity: Vec<u8>,
    user_id: i64,
    auth: Option<PendingAuth>,
}

impl Client {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .cookie_store(true)
                .build()?,
            pass_token: String::new(),
            ssecurity: Vec::new(),
            user_id: 0,
            auth: None,
        })
    }

    pub fn token(&self) -> String {
        format!("{}:{}", self.user_id, self.pass_token)
    }

    pub async fn request(
        &self,
        base_url: &str,
        api_url: &str,
        params: &str,
        headers: &HashMap<String, String>,
    ) -> Result<Vec<u8>> {
        let ssecurity64 = STANDARD.encode(&self.ssecurity);
        let encrypted = generate_encrypted_params(
            api_url,
            "POST",
            &ssecurity64,
            vec![("data".to_string(), params.to_string())],
        )?;
        let body = encode_form(&encrypted.form);
        let mut request = self
            .client
            .post(format!("{}{}", base_url, api_url))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(body);

        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        if !response.status().is_success() {
            let error = XiaomiError::HttpStatus(response.status().to_string());

            return Err(error);
        }

        let body = response.bytes().await?;
        let body = std::str::from_utf8(body.as_ref())
            .map_err(|err| XiaomiError::Auth(format!("response body is not UTF-8: {err}")))?;
        let plaintext = decrypt_response_payload(&ssecurity64, &encrypted.nonce64, body)?;
        let response: ApiResponse = serde_json::from_str(&plaintext)?;

        if response.code != 0 {
            let error = XiaomiError::Api(response.message);

            return Err(error);
        }

        Ok(response.result.get().as_bytes().to_vec())
    }
}

#[derive(serde::Deserialize)]
struct ApiResponse {
    code: i32,
    message: String,
    result: Box<RawValue>,
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
