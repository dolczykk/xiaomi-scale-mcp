use crate::base::{Client, Result};
use crate::encryption::{decrypt_response_payload, generate_encrypted_params};
use crate::errors::XiaomiError;
use crate::utils::{encode_form, normalize_api_signature_uri};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod account;
pub mod devices;
mod utils;
pub mod weight;

const XIAOMI_HOME_BASE_API: &str = "https://{}.api.io.mi.com/app";
const XIAOMI_HOME_CORE_BASE_API: &str = "https://{}.core.api.io.mi.com/app/v2";

#[derive(Debug, Deserialize)]
#[serde(bound = "T: DeserializeOwned")]
pub struct XiaomiHomeResponse<T: DeserializeOwned> {
    pub code: i32,
    pub message: String,
    pub result: T,
}

impl<T: DeserializeOwned> XiaomiHomeResponse<T> {
    fn from_json(json: &str) -> Result<Self> {
        let response: XiaomiHomeResponse<T> = serde_json::from_str(json)?;

        Ok(response)
    }
}

impl Client {
    pub async fn home_request<TRequest: Serialize, TResponse: DeserializeOwned>(
        &self,
        base_url: &str,
        api_url: &str,
        params: &TRequest,
        headers: &HashMap<String, String>,
    ) -> Result<XiaomiHomeResponse<TResponse>> {
        let ssecurity64 = STANDARD.encode(&self.ssecurity);
        let signature_uri = normalize_api_signature_uri(base_url, api_url)?;

        let json = serde_json::to_string(params)?;
        let encrypted_params = generate_encrypted_params(
            &signature_uri,
            "POST",
            &ssecurity64,
            vec![("data".to_string(), json)],
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

        let response: XiaomiHomeResponse<TResponse> =
            XiaomiHomeResponse::from_json(plaintext.as_str())?;
        if response.code != 0 {
            let error = XiaomiError::Api(response.message);

            return Err(error);
        }

        Ok(response)
    }
}
