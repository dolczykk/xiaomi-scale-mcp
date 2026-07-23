use crate::Client;
use crate::encryption::{decrypt_response_payload, generate_encrypted_params};
use crate::errors::XiaomiError;
use crate::utils::{encode_form, normalize_api_signature_uri};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

pub mod devices;

const XIAOMI_HOME_CORE_BASE_API: &str = "https://{}.core.api.io.mi.com/app/v2";

pub fn get_xiaomi_home_api_url(region: &str) -> String {
    XIAOMI_HOME_CORE_BASE_API.replace("{}", region)
}

#[derive(Debug, Deserialize)]
#[serde(bound = "T: DeserializeOwned")]
pub struct XiaomiHomeResponse<T: DeserializeOwned> {
    pub code: i32,
    pub message: String,
    pub result: Box<T>,
}

impl<T: DeserializeOwned> XiaomiHomeResponse<T> {
    fn from_json(json: &str) -> crate::Result<Self> {
        let response: XiaomiHomeResponse<T> = serde_json::from_str(json)?;

        Ok(response)
    }
}

impl Client {
    pub async fn home_request<T: DeserializeOwned>(
        &self,
        base_url: &str,
        api_url: &str,
        params: &str,
        headers: &HashMap<String, String>,
    ) -> crate::Result<XiaomiHomeResponse<T>> {
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
