use crate::auth::LoginChallenge;

#[derive(Debug, thiserror::Error)]
pub enum XiaomiError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("crypto error")]
    Crypto,

    #[error("wrong loginPrefix")]
    WrongLoginPrefix,

    #[error("missing redirect location")]
    MissingLocation,

    #[error("invalid token")]
    InvalidToken,

    #[error("wrong login step: {0}")]
    InvalidLoginStep(String),

    #[error("login challenge: {0:?}")]
    LoginChallenge(LoginChallenge),

    #[error("auth: {0}")]
    Auth(String),

    #[error("api: {0}")]
    Api(String),

    #[error("http status: {0}")]
    HttpStatus(String),
}

impl XiaomiError {
    pub(crate) fn invalid_login_step(message: impl Into<String>) -> Self {
        Self::InvalidLoginStep(message.into())
    }
}
