use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State as AxumState},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

pub(crate) async fn require_bearer_token(
    AxumState(expected_token): AxumState<Arc<String>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
        .is_some_and(|provided| bool::from(expected_token.as_bytes().ct_eq(provided.as_bytes())));

    if authorized {
        next.run(request).await
    } else {
        unauthorized_response()
    }
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.contains(char::is_whitespace)
    {
        return None;
    }

    Some(token)
}

fn unauthorized_response() -> Response {
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

#[cfg(test)]
mod tests {
    use super::parse_bearer_token;

    #[test]
    fn parses_bearer_token_case_insensitively() {
        assert_eq!(parse_bearer_token("bearer secret"), Some("secret"));
    }

    #[test]
    fn rejects_invalid_bearer_values() {
        for value in ["", "Basic secret", "Bearer", "Bearer ", "Bearer two words"] {
            assert_eq!(parse_bearer_token(value), None);
        }
    }
}
