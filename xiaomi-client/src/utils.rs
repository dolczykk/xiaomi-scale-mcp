use rand::RngExt;
use url::form_urlencoded::Serializer;

use crate::{LOGIN_PREFIX, Result, errors::XiaomiError};

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

pub fn crypt(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    use rc4::KeyInit;
    use rc4::StreamCipher;

    let mut cipher = rc4::Rc4::new_from_slice(key).map_err(|_| XiaomiError::Crypto)?;

    let mut drop = vec![0_u8; 1024];
    cipher.apply_keystream(&mut drop);

    let mut ciphertext = plaintext.to_vec();
    cipher.apply_keystream(&mut ciphertext);

    Ok(ciphertext)
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

pub fn encode_form(values: &[(String, String)]) -> String {
    let mut serializer = Serializer::new(String::new());

    for (key, value) in values {
        serializer.append_pair(key, value);
    }

    serializer.finish()
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
