use std::collections::HashMap;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::header::CONTENT_TYPE;
use serde_json::value::RawValue;

use crate::xiaomi::{
    auth::PendingAuth,
    errors::XiaomiError,
    utils::{crypt, encode_form, gen_nonce, gen_signature64, gen_signed_nonce},
};

pub mod auth;
mod cookies;
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
        let mut form = vec![("data".to_string(), params.to_string())];

        let nonce = gen_nonce();
        let signed_nonce = gen_signed_nonce(&self.ssecurity, &nonce);

        let rc4_hash = gen_signature64("POST", api_url, &form, &signed_nonce);
        form.push(("rc4_hash__".to_string(), rc4_hash));

        for (_, value) in &mut form {
            let ciphertext = crypt(&signed_nonce, value.as_bytes())?;
            *value = STANDARD.encode(ciphertext);
        }

        let signature = gen_signature64("POST", api_url, &form, &signed_nonce);
        form.push(("signature".to_string(), signature));
        form.push(("_nonce".to_string(), STANDARD.encode(nonce)));

        let body = encode_form(&form);
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
        let ciphertext = STANDARD.decode(body.as_ref())?;
        let plaintext = crypt(&signed_nonce, &ciphertext)?;
        let response: ApiResponse = serde_json::from_slice(&plaintext)?;

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
    use crate::xiaomi::utils::strip_login_prefix;

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

    #[test]
    fn generates_signed_nonce() {
        let signed = gen_signed_nonce(b"ssecurity", b"nonce");
        assert_eq!(
            STANDARD.encode(signed),
            "/oX2A3COQbfnXTsssP9J8BTo+5jiwum99lk9VJaElVI="
        );
    }

    #[test]
    fn crypt_round_trips() {
        let key = [7_u8; 32];
        let plaintext = b"hello xiaomi";
        let ciphertext = crypt(&key, plaintext).unwrap();
        assert_ne!(ciphertext, plaintext);
        assert_eq!(crypt(&key, &ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn signature_matches_expected_value() {
        let values = vec![("data".to_string(), "{\"x\":1}".to_string())];
        let signed_nonce = b"12345678901234567890123456789012";

        assert_eq!(
            gen_signature64("POST", "/app/test", &values, signed_nonce),
            "mqtlSvRzIbROSU2EFkWdWlHTCRE="
        );
    }
}
