mod parser;

use xiaomi_client::encryption::{decrypt_response_payload, generate_encrypted_params_with_nonce};
use xiaomi_client::utils::encode_form;

use crate::parser::{
    Cli, Command, DecryptArgs, EncryptArgs, optional_base64_string_or_prompt, parse_form,
    prompt_command, request_nonce64, required_base64_string, required_value_or_prompt,
    value_or_prompt_default,
};

fn encrypt(mut args: EncryptArgs) -> anyhow::Result<()> {
    let ssecurity = required_base64_string(&mut args.ssecurity, "ssecurity", "ssecurity base64")?;
    let api_url = required_value_or_prompt(&mut args.api_url, "API URL/path")?;
    let data = required_value_or_prompt(&mut args.data, "Plaintext data")?;
    let method = value_or_prompt_default(&mut args.method, "HTTP method", "POST")?;
    let nonce = optional_base64_string_or_prompt(
        &mut args.nonce,
        "nonce",
        "Nonce base64 (blank to generate)",
    )?;
    let encrypted = generate_encrypted_params_with_nonce(
        &api_url,
        &method,
        &ssecurity,
        nonce.as_deref(),
        vec![("data".to_string(), data)],
    )?;

    println!("{}", encode_form(&encrypted.form));

    Ok(())
}

fn decrypt(mut args: DecryptArgs) -> anyhow::Result<()> {
    let ssecurity = required_base64_string(&mut args.ssecurity, "ssecurity", "ssecurity base64")?;
    let form = required_value_or_prompt(&mut args.form, "Original URL-encoded request form")?;
    let body = required_value_or_prompt(&mut args.body, "Base64 response body")?;
    let values = parse_form(&form);
    let nonce = request_nonce64(&values)?;
    let plaintext = decrypt_response_payload(&ssecurity, nonce, &body)?;

    println!("{plaintext}");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let command = match Cli::parse_args().command {
        Some(command) => command,
        None => prompt_command()?,
    };

    match command {
        Command::Encrypt(args) => encrypt(args),
        Command::Decrypt(args) => decrypt(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use xiaomi_client::encryption::{encrypt_bytes, signed_nonce_from_base64};

    #[test]
    fn decrypts_response_body_using_nonce_from_request_form() {
        let ssecurity = "c3NlY3VyaXR5";
        let nonce = "MTIzNDU2Nzg5MDEy";
        let form = encode_form(&[("_nonce".to_string(), nonce.to_string())]);
        let signed_nonce = signed_nonce_from_base64(ssecurity, nonce).unwrap();
        let ciphertext =
            encrypt_bytes(&signed_nonce, br#"{"code":0,"result":{"ok":true}}"#).unwrap();
        let body = STANDARD.encode(ciphertext);

        let values = parse_form(&form);
        let nonce = request_nonce64(&values).unwrap();
        assert_eq!(
            decrypt_response_payload(ssecurity, nonce, &body).unwrap(),
            r#"{"code":0,"result":{"ok":true}}"#
        );
    }
}

