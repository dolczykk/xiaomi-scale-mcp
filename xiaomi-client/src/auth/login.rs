use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, COOKIE, LOCATION};
use serde::Deserialize;
use url::Url;

use super::{
    LoginChallenge, LoginV1Response, LoginV2Outcome, LoginV2Response, OAUTH2_AUTHORIZE_URL,
    PendingAuth, SERVICE_LOGIN_AUTH2_URL, SERVICE_LOGIN_URL,
    cookies::{find_cookie, parse_extension_ssecurity},
    parse_login_v2_outcome, parse_login_v2_response,
};
use crate::{
    base::{Client, Result},
    errors::XiaomiError,
    utils::{encode_form, read_login_response},
};

const ACCOUNT_BASE_URL: &str = "https://account.xiaomi.com";

impl Client {
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        self.auth = Some(PendingAuth::new(username, password));

        let res1 = self.service_login().await?;
        let res2 = self.service_login2(&res1, username, password).await?;

        self.finish_auth(&res2.location).await
    }

    pub async fn login_with_token(&mut self, token: &str) -> Result<()> {
        let (user_id, pass_token) = token.split_once(':').ok_or(XiaomiError::InvalidToken)?;
        let url = format!(
            "{}?_json=true&appName=com.xiaomi.smarthome&sid={}&_locale={}",
            SERVICE_LOGIN_URL, self.sid, self.locale
        );

        let response = self
            .client
            .get(url)
            .header(
                COOKIE,
                format!(
                    "userId={}; passToken={}; deviceId={}",
                    user_id,
                    pass_token,
                    self.device_id.as_deref().unwrap_or_default()
                ),
            )
            .send()
            .await?;

        let body = read_login_response(response).await?;
        let res2 = parse_login_v2_response(&body)?;

        self.apply_login_response(&res2);

        self.finish_auth(&res2.location).await
    }

    pub async fn login_with_captcha(&mut self, captcha: &str) -> Result<()> {
        let pending = self
            .auth
            .as_mut()
            .ok_or_else(|| XiaomiError::invalid_login_step("captcha not requested"))?;

        if pending.ick.as_deref().unwrap_or_default().is_empty() {
            let error = XiaomiError::invalid_login_step("captcha not requested");

            return Err(error);
        }

        pending.captcha_code = Some(captcha.to_string());

        if pending.flag.is_some() {
            return self.send_ticket().await;
        }

        let username = pending.username.clone();
        let password = pending.password.clone();
        let res1 = self.service_login().await?;
        let res2 = self.service_login2(&res1, &username, &password).await?;
        self.finish_auth(&res2.location).await
    }

    pub async fn login_with_verify(&mut self, ticket: &str) -> Result<()> {
        let (flag, identity_session) = self.verification_details()?;
        let verification_name = verify_name(&flag)?;

        let form = vec![
            ("_flag".to_string(), flag),
            ("ticket".to_string(), ticket.to_string()),
            ("trust".to_string(), "false".to_string()),
            ("_json".to_string(), "true".to_string()),
        ];

        let response = self
            .client
            .post(format!(
                "{}/identity/auth/verify{}",
                ACCOUNT_BASE_URL, verification_name
            ))
            .header(COOKIE, format!("identity_session={identity_session}"))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(encode_form(&form))
            .send()
            .await?;

        let body = read_login_response(response).await?;
        let response: VerifyResponse = serde_json::from_slice(&body)?;

        if response.location.is_empty() {
            let error = XiaomiError::Auth(raw_login_error(&body));

            return Err(error);
        }

        self.finish_auth(&response.location).await
    }

    pub async fn oauth2(&mut self, params: &str, username: &str, password: &str) -> Result<String> {
        self.auth = Some(PendingAuth::new(username, password));

        let res1 = self.oauth2_authorize(params).await?;
        let res2 = self.service_login2(&res1, username, password).await?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() == 2 {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()?;

        let response = client.get(res2.location).send().await?;
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(XiaomiError::MissingLocation)?;
        let (_, code) = location
            .split_once('=')
            .ok_or(XiaomiError::MissingLocation)?;

        Ok(code.to_string())
    }

    async fn service_login(&self) -> Result<LoginV1Response> {
        let url = format!("{}?_json=true&sid={}", SERVICE_LOGIN_URL, self.sid);
        let response = self.client.get(url).send().await?;
        let body = read_login_response(response).await?;

        Ok(serde_json::from_slice(&body)?)
    }

    async fn service_login2(
        &mut self,
        res1: &LoginV1Response,
        username: &str,
        password: &str,
    ) -> Result<LoginV2Response> {
        let hash = format!("{:X}", md5::compute(password.as_bytes()));
        let mut form = vec![
            ("_json".to_string(), "true".to_string()),
            ("hash".to_string(), hash),
            ("sid".to_string(), res1.sid.clone()),
            ("callback".to_string(), res1.callback.clone()),
            ("_sign".to_string(), res1.sign.clone()),
            ("qs".to_string(), res1.qs.clone()),
            ("user".to_string(), username.to_string()),
        ];

        let device_id = self.device_id.clone().unwrap_or_default();

        let mut cookies = format!("deviceId={}", device_id.as_str());
        if let Some(auth) = &self.auth {
            if let Some(captcha_code) = &auth.captcha_code {
                form.push(("captCode".to_string(), captcha_code.clone()));
            }
            if let Some(ick) = &auth.ick {
                cookies.push_str("; ick=");
                cookies.push_str(ick);
            }
        }

        let response = self
            .client
            .post(SERVICE_LOGIN_AUTH2_URL)
            .header(COOKIE, cookies)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(encode_form(&form))
            .send()
            .await?;

        let body = read_login_response(response).await?;
        match parse_login_v2_outcome(&body)? {
            LoginV2Outcome::Success(response) => {
                self.apply_login_response(&response);
                Ok(response)
            }
            LoginV2Outcome::Captcha(captcha_url) => {
                self.get_captcha(&captcha_url).await?;
                let error = XiaomiError::Auth("captcha challenge did not return".to_string());

                Err(error)
            }
            LoginV2Outcome::Notification(notification_url) => {
                self.auth_start(&notification_url).await?;
                let error = XiaomiError::Auth("verification challenge did not return".to_string());

                Err(error)
            }
        }
    }

    async fn get_captcha(&mut self, captcha_url: &str) -> Result<()> {
        let url = Url::parse(ACCOUNT_BASE_URL)?.join(captcha_url)?;

        let response = self.client.get(url).send().await?;
        let ick = find_cookie(response.headers(), "ick");
        let captcha = response.bytes().await?.to_vec();

        if let Some(auth) = &mut self.auth {
            auth.ick = ick;
        }

        let challenge = LoginChallenge::captcha(captcha);
        let error = XiaomiError::LoginChallenge(challenge);

        Err(error)
    }

    async fn auth_start(&mut self, notification_url: &str) -> Result<()> {
        let url = notification_url.replace("/fe/service/identity/authStart", "/identity/list");

        let response = self.client.get(url).send().await?;

        let identity_session = find_cookie(response.headers(), "identity_session");
        let body = read_login_response(response).await?;
        let response: IdentityListResponse = serde_json::from_slice(&body)?;

        let Some(auth) = &mut self.auth else {
            let error = XiaomiError::invalid_login_step("login state missing");

            return Err(error);
        };

        auth.flag = Some(response.flag.to_string());
        auth.identity_session = identity_session;

        self.send_ticket().await
    }

    async fn send_ticket(&mut self) -> Result<()> {
        let (flag, identity_session) = self.verification_details()?;
        let name = verify_name(&flag)?;

        let response = self
            .client
            .get(format!(
                "{ACCOUNT_BASE_URL}/identity/auth/verify{name}?_flag={flag}&_json=true"
            ))
            .header(COOKIE, format!("identity_session={identity_session}"))
            .send()
            .await?;

        let body = read_login_response(response).await?;
        let verify: VerifyMethodResponse = serde_json::from_slice(&body)?;

        let pending = self.pending_auth()?;
        let captcha_code = pending.captcha_code.clone().unwrap_or_default();
        let mut cookies = format!("identity_session={identity_session}");

        if !captcha_code.is_empty()
            && let Some(ick) = &pending.ick
        {
            cookies.push_str("; ick=");
            cookies.push_str(ick);
        }

        let form = vec![
            ("_json".to_string(), "true".to_string()),
            ("icode".to_string(), captcha_code),
            ("retry".to_string(), "0".to_string()),
        ];

        let response = self
            .client
            .post(format!("{ACCOUNT_BASE_URL}/identity/auth/send{name}Ticket"))
            .header(COOKIE, cookies)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(encode_form(&form))
            .send()
            .await?;

        let body = read_login_response(response).await?;
        let ticket: SendTicketResponse = serde_json::from_slice(&body)?;

        if let Some(captcha_url) = ticket.captcha_url.filter(|value| !value.is_empty()) {
            return self.get_captcha(&captcha_url).await;
        }

        if ticket.code != 0 {
            let error = XiaomiError::Auth(raw_login_error(&body));

            return Err(error);
        }

        let challenge = LoginChallenge::verification(verify.masked_phone, verify.masked_email);
        let error = XiaomiError::LoginChallenge(challenge);

        Err(error)
    }

    async fn finish_auth(&mut self, location: &str) -> Result<()> {
        if location.is_empty() {
            let error = XiaomiError::MissingLocation;

            return Err(error);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let mut url = Url::parse(location)?;
        let mut c_user_id = String::new();
        let mut service_token = String::new();

        for _ in 0..10 {
            let mut request = client.get(url.clone());
            if self.user_id != 0 && !self.pass_token.is_empty() {
                request = request.header(
                    COOKIE,
                    format!("userId={}; passToken={}", self.user_id, self.pass_token),
                );
            }

            let response = request.send().await?;

            if let Some(user_id) = find_cookie(response.headers(), "userId")
                && let Ok(user_id) = user_id.parse::<i64>()
            {
                self.user_id = user_id;
            }
            if let Some(value) = find_cookie(response.headers(), "cUserId") {
                c_user_id = value;
            }
            if let Some(value) = find_cookie(response.headers(), "serviceToken") {
                service_token = value;
            }
            if let Some(value) = find_cookie(response.headers(), "passToken") {
                self.pass_token = value;
            }
            if let Some(ssecurity) = parse_extension_ssecurity(response.headers())? {
                self.ssecurity = ssecurity;
            }

            if !response.status().is_redirection() {
                break;
            }

            let Some(next) = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
            else {
                break;
            };
            url = url.join(next)?;
        }

        self.c_user_id = c_user_id;
        self.service_token = service_token;
        self.auth = None;

        Ok(())
    }

    async fn oauth2_authorize(&self, params: &str) -> Result<LoginV1Response> {
        let response = self
            .client
            .get(format!("{OAUTH2_AUTHORIZE_URL}?{params}"))
            .send()
            .await?;
        let body = read_login_response(response).await?;
        let response: OAuthAuthorizeResponse = serde_json::from_slice(&body)?;

        let response = self
            .client
            .get(response.data.oauth_login_url)
            .send()
            .await?;
        let body = read_login_response(response).await?;

        Ok(serde_json::from_slice(&body)?)
    }
}

impl Client {
    fn apply_login_response(&mut self, response: &LoginV2Response) {
        self.pass_token.clone_from(&response.pass_token);
        self.ssecurity.clone_from(&response.ssecurity);
        self.user_id = response.user_id;
    }

    fn pending_auth(&self) -> Result<&PendingAuth> {
        self.auth
            .as_ref()
            .ok_or_else(|| XiaomiError::invalid_login_step("verification not requested"))
    }

    fn verification_details(&self) -> Result<(String, String)> {
        let (flag, identity_session) = self.pending_auth()?.verification_details()?;

        Ok((flag.to_owned(), identity_session.to_owned()))
    }
}

fn verify_name(flag: &str) -> Result<&'static str> {
    match flag {
        "4" => Ok("Phone"),
        "8" => Ok("Email"),
        _ => Err(XiaomiError::invalid_login_step(
            "unsupported verification type",
        )),
    }
}

fn raw_login_error(body: &[u8]) -> String {
    String::from_utf8_lossy(body).into_owned()
}

#[derive(Deserialize)]
struct IdentityListResponse {
    flag: i64,
}

#[derive(Deserialize)]
struct VerifyMethodResponse {
    #[serde(rename = "maskedPhone")]
    masked_phone: Option<String>,

    #[serde(rename = "maskedEmail")]
    masked_email: Option<String>,
}

#[derive(Deserialize)]
struct SendTicketResponse {
    code: i32,

    #[serde(rename = "captchaURL", alias = "captchaUrl")]
    captcha_url: Option<String>,
}

#[derive(Deserialize)]
struct VerifyResponse {
    #[serde(default)]
    location: String,
}

#[derive(Deserialize)]
struct OAuthAuthorizeResponse {
    data: OAuthAuthorizeData,
}

#[derive(Deserialize)]
struct OAuthAuthorizeData {
    #[serde(rename = "oauthLoginUrl")]
    oauth_login_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_verification_names() {
        assert_eq!(verify_name("4").unwrap(), "Phone");
        assert_eq!(verify_name("8").unwrap(), "Email");
    }

    #[test]
    fn rejects_unsupported_verification_name() {
        assert!(matches!(
            verify_name("2"),
            Err(XiaomiError::InvalidLoginStep(message))
                if message == "unsupported verification type"
        ));
    }
}
