use crate::{base::Result, errors::XiaomiError};
use reqwest::header::{HeaderMap, SET_COOKIE};
use serde::Deserialize;

#[derive(Deserialize)]
struct ExtensionPragma {
    #[serde(deserialize_with = "crate::utils::serde_base64::deserialize")]
    ssecurity: Vec<u8>,
}

pub(crate) fn find_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(SET_COOKIE).iter().find_map(|value| {
        value
            .to_str()
            .ok()
            .and_then(cookie_name_value)
            .and_then(|(cookie_name, cookie_value)| (cookie_name == name).then_some(cookie_value))
    })
}

pub(crate) fn parse_extension_ssecurity(headers: &HeaderMap) -> Result<Option<Vec<u8>>> {
    let Some(value) = headers.get("Extension-Pragma") else {
        return Ok(None);
    };

    let value = value
        .to_str()
        .map_err(|err| XiaomiError::Auth(err.to_string()))?;
    let extension: ExtensionPragma = serde_json::from_str(value)?;

    Ok(Some(extension.ssecurity))
}

fn cookie_name_value(cookie: &str) -> Option<(String, String)> {
    let cookie = cookie.split_once(';').map_or(cookie, |(cookie, _)| cookie);
    let (name, value) = cookie.split_once('=')?;

    Some((name.to_string(), value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn finds_named_cookie() {
        let mut headers = HeaderMap::new();
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("identity_session=session; Path=/; HttpOnly"),
        );

        assert_eq!(
            find_cookie(&headers, "identity_session").as_deref(),
            Some("session")
        );
    }

    #[test]
    fn parses_extension_ssecurity() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Extension-Pragma",
            HeaderValue::from_static(r#"{"ssecurity":"AQIDBA=="}"#),
        );

        assert_eq!(
            parse_extension_ssecurity(&headers).unwrap(),
            Some(vec![1, 2, 3, 4])
        );
    }
}
