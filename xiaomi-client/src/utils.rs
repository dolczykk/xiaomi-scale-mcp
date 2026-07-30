use std::env;

use chrono::{Datelike, Local, TimeZone};
use rand::RngExt;
use url::{Url, form_urlencoded::Serializer};

use crate::{auth::LOGIN_PREFIX, base::Result, errors::XiaomiError};

pub async fn read_login_response(response: reqwest::Response) -> Result<Vec<u8>> {
    strip_login_prefix(&response.bytes().await?)
}

pub fn strip_login_prefix(body: &[u8]) -> Result<Vec<u8>> {
    if !body.starts_with(LOGIN_PREFIX.as_bytes()) {
        let error = XiaomiError::WrongLoginPrefix;

        return Err(error);
    }

    Ok(body[LOGIN_PREFIX.len()..].to_vec())
}

pub fn random_string(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn random_lowercase_string(len: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn encode_form(values: &[(String, String)]) -> String {
    let mut serializer = Serializer::new(String::new());

    for (key, value) in values {
        serializer.append_pair(key, value);
    }

    serializer.finish()
}

pub fn normalize_api_signature_uri(base_url: &str, api_url: &str) -> Result<String> {
    let mut full_url = String::new();
    full_url.push_str(base_url.trim_end_matches('/'));
    full_url.push('/');
    full_url.push_str(api_url.trim_start_matches('/'));

    let url = Url::parse(&full_url)?;
    Ok(url.path().replace("/app/", "/"))
}

pub fn local_timezone_name() -> String {
    env::var("TZ").unwrap_or_else(|_| "local".to_string())
}

pub fn local_timezone_offset() -> String {
    let offset_seconds = Local::now().offset().local_minus_utc();
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let offset_seconds = offset_seconds.abs();
    let hours = offset_seconds / 3600;
    let minutes = offset_seconds % 3600 / 60;

    format!("{sign}{hours:02}:{minutes:02}")
}

pub fn is_daylight_saving_time() -> bool {
    let now = Local::now();
    let year = now.year();
    let current_offset = now.offset().local_minus_utc();

    let january_offset = Local
        .with_ymd_and_hms(year, 1, 1, 12, 0, 0)
        .single()
        .map(|date| date.offset().local_minus_utc());

    let july_offset = Local
        .with_ymd_and_hms(year, 7, 1, 12, 0, 0)
        .single()
        .map(|date| date.offset().local_minus_utc());

    let standard_offset = [january_offset, july_offset]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(current_offset);

    current_offset > standard_offset
}

pub(crate) mod serde_base64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }

    pub(crate) fn deserialize_optional<'de, D>(
        deserializer: D,
    ) -> std::result::Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = Option::<String>::deserialize(deserializer)?;
        encoded
            .map(|encoded| STANDARD.decode(encoded).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_api_signature_uri_from_full_request_parts() {
        assert_eq!(
            normalize_api_signature_uri(
                "https://de.core.api.io.mi.com/app/v2",
                "/home/device_list_page"
            )
            .unwrap(),
            "/v2/home/device_list_page"
        );
    }

    #[test]
    fn normalizes_api_signature_uri_without_duplicate_slashes() {
        assert_eq!(
            normalize_api_signature_uri("https://de.api.io.mi.com/app", "v2/home/home_device_list")
                .unwrap(),
            "/v2/home/home_device_list"
        );
    }
}
