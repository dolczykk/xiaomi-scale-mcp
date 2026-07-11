use serde::Deserialize;

use crate::xiaomi::{Result, errors::XiaomiError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginChallenge {
    pub captcha: Option<Vec<u8>>,
    pub verify_phone: Option<String>,
    pub verify_email: Option<String>,
}

impl LoginChallenge {
    pub fn captcha(captcha: Vec<u8>) -> Self {
        Self {
            captcha: Some(captcha),
            verify_phone: None,
            verify_email: None,
        }
    }

    pub fn verification(verify_phone: Option<String>, verify_email: Option<String>) -> Self {
        Self {
            captcha: None,
            verify_phone,
            verify_email,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingAuth {
    pub username: String,
    pub password: String,
    pub ick: Option<String>,
    pub flag: Option<String>,
    pub identity_session: Option<String>,
    pub captcha_code: Option<String>,
}

impl PendingAuth {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            ick: None,
            flag: None,
            identity_session: None,
            captcha_code: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginV1Response {
    pub qs: String,

    #[serde(rename = "_sign")]
    pub sign: String,

    pub sid: String,

    pub callback: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginV2Response {
    pub user_id: i64,

    #[serde(deserialize_with = "crate::xiaomi::utils::serde_base64::deserialize")]
    pub ssecurity: Vec<u8>,

    pub pass_token: String,

    pub location: String,
}

#[derive(Debug)]
pub(crate) enum LoginV2Outcome {
    Success(LoginV2Response),
    Captcha(String),
    Notification(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginV2Envelope {
    code: Option<i64>,
    result: Option<String>,
    description: Option<String>,
    desc: Option<String>,
    reason: Option<String>,

    user_id: Option<i64>,

    #[serde(
        default,
        deserialize_with = "crate::xiaomi::utils::serde_base64::deserialize_optional"
    )]
    ssecurity: Option<Vec<u8>>,

    pass_token: Option<String>,

    notification_url: Option<String>,

    #[serde(rename = "captchaURL", alias = "captchaUrl")]
    captcha_url: Option<String>,

    location: Option<String>,
}

pub fn parse_login_v2_response(body: &[u8]) -> Result<LoginV2Response> {
    match parse_login_v2_outcome(body)? {
        LoginV2Outcome::Success(response) => Ok(response),
        LoginV2Outcome::Captcha(captcha_url) => Err(XiaomiError::Auth(format!(
            "captcha required: {captcha_url}"
        ))),
        LoginV2Outcome::Notification(notification_url) => Err(XiaomiError::Auth(format!(
            "notification required: {notification_url}"
        ))),
    }
}

pub(crate) fn parse_login_v2_outcome(body: &[u8]) -> Result<LoginV2Outcome> {
    let response = serde_json::from_slice::<LoginV2Envelope>(body)?;

    if let Some(captcha_url) = response
        .captcha_url
        .clone()
        .filter(|value| !value.is_empty())
    {
        return Ok(LoginV2Outcome::Captcha(captcha_url));
    }

    if let Some(notification_url) = response
        .notification_url
        .clone()
        .filter(|value| !value.is_empty())
    {
        return Ok(LoginV2Outcome::Notification(notification_url));
    }

    if let (Some(user_id), Some(ssecurity), Some(pass_token), Some(location)) = (
        response.user_id,
        response.ssecurity.clone(),
        response.pass_token.clone(),
        response.location.clone(),
    ) && !location.is_empty()
    {
        return Ok(LoginV2Outcome::Success(LoginV2Response {
            user_id,
            ssecurity,
            pass_token,
            location,
        }));
    }

    Err(XiaomiError::Auth(response.error_message()))
}

impl LoginV2Envelope {
    fn error_message(self) -> String {
        let detail = self
            .description
            .or(self.desc)
            .or(self.reason)
            .or(self.result)
            .unwrap_or_else(|| "login failed".to_string());

        let mut parts = Vec::new();
        if let Some(code) = self.code {
            parts.push(format!("code {code}"));
        }
        parts.push(detail);

        if let Some(notification_url) = self.notification_url {
            parts.push(format!("notification required: {notification_url}"));
        }

        if let Some(captcha_url) = self.captcha_url {
            parts.push(format!("captcha required: {captcha_url}"));
        }

        if let Some(location) = self.location {
            parts.push(format!("location: {location}"));
        }

        parts.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_login_v2_ssecurity() {
        let response = serde_json::from_str::<LoginV2Response>(
            r#"{
                "userId": 42,
                "ssecurity": "AQIDBA==",
                "passToken": "pass",
                "location": "https://example.com"
            }"#,
        )
        .unwrap();

        assert_eq!(response.user_id, 42);
        assert_eq!(response.ssecurity, vec![1, 2, 3, 4]);
        assert_eq!(response.pass_token, "pass");
        assert_eq!(response.location, "https://example.com");
    }

    #[test]
    fn reports_login_error_response_without_challenge() {
        let error = parse_login_v2_response(
            br#"{
                "code": 70016,
                "description": "Invalid credential"
            }"#,
        )
        .unwrap_err();

        assert!(matches!(error, XiaomiError::Auth(_)));
        assert!(error.to_string().contains("Invalid credential"));
    }

    #[test]
    fn classifies_captcha_response() {
        let outcome = parse_login_v2_outcome(br#"{"captchaURL":"/pass/getCode?icode=1"}"#).unwrap();

        assert!(matches!(
            outcome,
            LoginV2Outcome::Captcha(ref url) if url == "/pass/getCode?icode=1"
        ));
    }

    #[test]
    fn classifies_notification_response() {
        let outcome =
            parse_login_v2_outcome(br#"{"notificationUrl":"https://account.xiaomi.com/notice"}"#)
                .unwrap();

        assert!(matches!(
            outcome,
            LoginV2Outcome::Notification(ref url) if url == "https://account.xiaomi.com/notice"
        ));
    }

    #[test]
    fn rejects_empty_login_location() {
        let error = parse_login_v2_response(
            br#"{
                "userId": 42,
                "ssecurity": "AQIDBA==",
                "passToken": "pass",
                "location": ""
            }"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("login failed"));
        assert!(error.to_string().contains("location: "));
    }
}
