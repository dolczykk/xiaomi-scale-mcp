use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::read::GzDecoder;
use rand::RngExt;
use sha1::Digest as Sha1Digest;
use sha1::Sha1;

use crate::{base::Result, errors::XiaomiError};

#[must_use]
pub fn generate_nonce() -> Vec<u8> {
    let mut nonce = Vec::with_capacity(16);
    nonce.extend_from_slice(&rand::rng().random::<i64>().to_be_bytes());
    nonce.extend_from_slice(&minimal_be_bytes(current_millis() / 60_000));

    nonce
}

#[must_use]
pub fn generate_nonce64() -> String {
    STANDARD.encode(generate_nonce())
}

#[must_use]
pub fn signed_nonce(ssecurity: &[u8], nonce: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(ssecurity);
    hasher.update(nonce);

    hasher.finalize().to_vec()
}

pub fn signed_nonce_from_base64(ssecurity64: &str, nonce64: &str) -> Result<Vec<u8>> {
    let ssecurity = decode_base64(ssecurity64)?;
    let nonce = decode_base64(nonce64)?;

    Ok(signed_nonce(&ssecurity, &nonce))
}

pub fn signed_nonce64(ssecurity64: &str, nonce64: &str) -> Result<String> {
    Ok(STANDARD.encode(signed_nonce_from_base64(ssecurity64, nonce64)?))
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

pub fn encrypt_bytes(signed_nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    crypt(signed_nonce, plaintext)
}

pub fn decrypt_bytes(signed_nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    crypt(signed_nonce, ciphertext)
}

pub fn encrypt_rc4_base64(signed_nonce64: &str, plaintext: &str) -> Result<String> {
    let signed_nonce = decode_base64(signed_nonce64)?;
    let ciphertext = encrypt_bytes(&signed_nonce, plaintext.as_bytes())?;

    Ok(STANDARD.encode(ciphertext))
}

pub fn decrypt_rc4_base64(signed_nonce64: &str, payload64: &str) -> Result<Vec<u8>> {
    let signed_nonce = decode_base64(signed_nonce64)?;
    let ciphertext = decode_base64(payload64)?;

    decrypt_bytes(&signed_nonce, &ciphertext)
}

#[must_use]
pub fn gen_enc_signature(
    uri: &str,
    method: &str,
    signed_nonce64: &str,
    params: &[(String, String)],
) -> String {
    let mut signature_params = vec![method.to_uppercase(), uri.to_string()];
    signature_params.extend(params.iter().map(|(key, value)| format!("{key}={value}")));
    signature_params.push(signed_nonce64.to_string());

    let signature_string = signature_params.join("&");
    let mut hasher = Sha1::new();
    hasher.update(signature_string.as_bytes());
    STANDARD.encode(hasher.finalize())
}

pub fn generate_encrypted_params(
    uri: &str,
    method: &str,
    ssecurity64: &str,
    params: Vec<(String, String)>,
) -> Result<Vec<(String, String)>> {
    generate_encrypted_params_with_nonce(uri, method, ssecurity64, None, params)
}

pub fn generate_encrypted_params_with_nonce(
    uri: &str,
    method: &str,
    ssecurity64: &str,
    nonce64: Option<&str>,
    mut params: Vec<(String, String)>,
) -> Result<Vec<(String, String)>> {
    let nonce64 = nonce64.map_or_else(generate_nonce64, ToString::to_string);
    let signed_nonce64 = signed_nonce64(ssecurity64, &nonce64)?;

    let rc4_hash = gen_enc_signature(uri, method, &signed_nonce64, &params);
    params.push(("rc4_hash__".to_string(), rc4_hash));

    let mut form = Vec::with_capacity(params.len() + 3);
    for (key, value) in params {
        let value = encrypt_rc4_base64(&signed_nonce64, &value)?;
        form.push((key, value));
    }

    let signature = gen_enc_signature(uri, method, &signed_nonce64, &form);
    form.push(("signature".to_string(), signature));
    form.push(("ssecurity".to_string(), ssecurity64.to_string()));
    form.push(("_nonce".to_string(), nonce64));

    Ok(form)
}

pub fn decrypt_response_payload(
    ssecurity64: &str,
    nonce64: &str,
    payload64: &str,
) -> Result<String> {
    let signed_nonce64 = signed_nonce64(ssecurity64, nonce64)?;
    let decrypted = decrypt_rc4_base64(&signed_nonce64, payload64)?;

    decode_response_bytes(&decrypted)
}

pub fn decode_response_bytes(decrypted: &[u8]) -> Result<String> {
    match String::from_utf8(decrypted.to_vec()) {
        Ok(plaintext) => Ok(plaintext),
        Err(utf8_error) => {
            let mut decoder = GzDecoder::new(decrypted);
            let mut plaintext = String::new();
            decoder
                .read_to_string(&mut plaintext)
                .map_err(|gzip_error| XiaomiError::Auth(format!(
                    "decrypted response was neither UTF-8 nor gzip-compressed UTF-8: {utf8_error}; gzip error: {gzip_error}"
                )))?;

            Ok(plaintext)
        }
    }
}

fn decode_base64(value: &str) -> Result<Vec<u8>> {
    Ok(STANDARD.decode(strip_wrapping_quotes(value.trim()))?)
}

#[must_use]
pub fn strip_wrapping_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }

    value
}

fn current_millis() -> u128 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch");

    u128::from(duration.as_secs()) * 1000
        + u128::from(duration.subsec_nanos() + 500_000) / 1_000_000
}

fn minimal_be_bytes(value: u128) -> Vec<u8> {
    if value == 0 {
        return Vec::new();
    }

    let bit_len = u128::BITS - value.leading_zeros();
    let byte_len = bit_len.div_ceil(8) as usize;

    value.to_be_bytes()[u128::BITS as usize / 8 - byte_len..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    #[test]
    fn nonce_contains_random_i64_and_current_minute() {
        let minute_before = current_millis() / 60_000;
        let nonce = generate_nonce();
        let minute_after = current_millis() / 60_000;
        let suffix = &nonce[8..];
        let expected_before = minimal_be_bytes(minute_before);
        let expected_after = minimal_be_bytes(minute_after);

        assert!(suffix == expected_before.as_slice() || suffix == expected_after.as_slice());
    }

    #[test]
    fn signed_nonce_matches_python_reference() {
        assert_eq!(
            signed_nonce64("c3NlY3VyaXR5", "bm9uY2U=").unwrap(),
            "/oX2A3COQbfnXTsssP9J8BTo+5jiwum99lk9VJaElVI="
        );
    }

    #[test]
    fn rc4_round_trips() {
        let key = [7_u8; 32];
        let plaintext = b"hello xiaomi";
        let ciphertext = encrypt_bytes(&key, plaintext).unwrap();

        assert_ne!(ciphertext, plaintext);
        assert_eq!(decrypt_bytes(&key, &ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn encrypted_params_include_python_fields() {
        let encrypted = generate_encrypted_params_with_nonce(
            "/app/test",
            "post",
            "c3NlY3VyaXR5",
            Some("MTIzNDU2Nzg5MDEy"),
            vec![("data".to_string(), r#"{"value":1}"#.to_string())],
        )
        .unwrap();

        assert!(encrypted.iter().any(|(key, _)| key == "data"));
        assert!(encrypted.iter().any(|(key, _)| key == "rc4_hash__"));
        assert!(encrypted.iter().any(|(key, _)| key == "signature"));
        assert!(encrypted.iter().any(|(key, _)| key == "ssecurity"));
        assert!(encrypted.iter().any(|(key, _)| key == "_nonce"));
    }

    #[test]
    fn encrypt_request_output_can_be_decrypted() {
        let encrypted = generate_encrypted_params_with_nonce(
            "/app/test",
            "POST",
            "c3NlY3VyaXR5",
            Some("MTIzNDU2Nzg5MDEy"),
            vec![("data".to_string(), r#"{"value":1}"#.to_string())],
        )
        .unwrap();
        let data = encrypted
            .iter()
            .find_map(|(key, value)| (key == "data").then_some(value))
            .unwrap();
        let signed_nonce = signed_nonce64("c3NlY3VyaXR5", "MTIzNDU2Nzg5MDEy").unwrap();
        let plaintext = decrypt_rc4_base64(&signed_nonce, data).unwrap();

        assert_eq!(plaintext, br#"{"value":1}"#);
    }

    #[test]
    fn decrypts_plain_utf8_response_payload() {
        let signed_nonce = signed_nonce_from_base64("c3NlY3VyaXR5", "MTIzNDU2Nzg5MDEy").unwrap();
        let ciphertext =
            encrypt_bytes(&signed_nonce, br#"{"code":0,"result":{"ok":true}}"#).unwrap();
        let payload = STANDARD.encode(ciphertext);

        assert_eq!(
            decrypt_response_payload("c3NlY3VyaXR5", "MTIzNDU2Nzg5MDEy", &payload).unwrap(),
            r#"{"code":0,"result":{"ok":true}}"#
        );
    }

    #[test]
    fn decrypts_gzip_response_payload() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(br#"{"code":0,"result":{"ok":true}}"#)
            .unwrap();
        let compressed = encoder.finish().unwrap();
        let signed_nonce = signed_nonce_from_base64("c3NlY3VyaXR5", "MTIzNDU2Nzg5MDEy").unwrap();
        let ciphertext = encrypt_bytes(&signed_nonce, &compressed).unwrap();
        let payload = STANDARD.encode(ciphertext);

        assert_eq!(
            decrypt_response_payload("c3NlY3VyaXR5", "MTIzNDU2Nzg5MDEy", &payload).unwrap(),
            r#"{"code":0,"result":{"ok":true}}"#
        );
    }
}
