use std::io::{self, Read, Write};

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Args, Parser, Subcommand};
use url::form_urlencoded;
use xiaomi_client::encryption::strip_wrapping_quotes;

#[derive(Debug, Parser)]
#[command(version, about = "Encrypt and decrypt Xiaomi request payloads")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Encrypt Xiaomi request parameters.
    Encrypt(EncryptArgs),

    /// Decrypt a Xiaomi response body using the original request form.
    Decrypt(DecryptArgs),
}

#[derive(Debug, Default, Args)]
pub struct EncryptArgs {
    /// Base64 Xiaomi ssecurity value.
    #[arg(long)]
    pub ssecurity: Option<String>,

    /// Xiaomi API URL/path used in the request signature.
    #[arg(long)]
    pub api_url: Option<String>,

    /// Plaintext data payload, or '-' to read from stdin.
    #[arg(long)]
    pub data: Option<String>,

    /// Base64 nonce to reuse, or omit/blank to generate one.
    #[arg(long)]
    pub nonce: Option<String>,

    /// HTTP method used in the request signature.
    #[arg(long)]
    pub method: Option<String>,
}

#[derive(Debug, Default, Args)]
pub struct DecryptArgs {
    /// Base64 Xiaomi ssecurity value.
    #[arg(long)]
    pub ssecurity: Option<String>,

    /// Original URL-encoded Xiaomi request form, or '-' to read from stdin.
    #[arg(long)]
    pub form: Option<String>,

    /// Base64 encrypted response body, or '-' to read from stdin.
    #[arg(long)]
    pub body: Option<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

pub fn required_or_prompt(value: &mut Option<String>, label: &str) -> Result<String> {
    if value.is_none() {
        let input = prompt_line(label)?;
        if input.trim().is_empty() {
            bail!("missing required value: {label}");
        }
        *value = Some(input);
    }

    Ok(value.clone().expect("value is present after prompt"))
}

pub fn value_or_prompt_default(
    value: &mut Option<String>,
    label: &str,
    default: &str,
) -> Result<String> {
    if value.is_none() {
        let input = prompt_line(&format!("{label} [{default}]"))?;
        *value = Some(if input.trim().is_empty() {
            default.to_string()
        } else {
            input
        });
    }

    Ok(value.clone().expect("value is present after prompt"))
}

pub fn required_value_or_prompt(value: &mut Option<String>, label: &str) -> Result<String> {
    let value = required_or_prompt(value, label)?;
    if value == "-" {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read stdin")?;
        Ok(input)
    } else {
        Ok(value)
    }
}

pub fn required_base64_string(
    value: &mut Option<String>,
    name: &str,
    label: &str,
) -> Result<String> {
    let value = required_or_prompt(value, label)?;
    decode_base64(&value, name)?;
    Ok(value)
}

pub fn optional_base64_string_or_prompt(
    value: &mut Option<String>,
    name: &str,
    label: &str,
) -> Result<Option<String>> {
    if value.is_none() {
        let input = prompt_line(label)?;
        if input.trim().is_empty() {
            return Ok(None);
        }
        *value = Some(input);
    }

    let value = value.clone();
    if let Some(value) = &value {
        decode_base64(value, name)?;
    }

    Ok(value)
}

pub fn parse_form(form: &str) -> Vec<(String, String)> {
    form_urlencoded::parse(form.trim().as_bytes())
        .into_owned()
        .collect()
}

pub fn request_nonce64(values: &[(String, String)]) -> Result<&str> {
    form_value(values, "_nonce").context("missing required form field: _nonce")
}

pub fn prompt_command() -> Result<Command> {
    println!("Select Xiaomi encryption operation:");
    println!("  1) encrypt");
    println!("  2) decrypt");

    let command = prompt_line("Operation")?;
    match command.trim() {
        "1" | "encrypt" => Ok(Command::Encrypt(EncryptArgs::default())),
        "2" | "decrypt" => Ok(Command::Decrypt(DecryptArgs::default())),
        value => bail!("unknown operation: {value}"),
    }
}

fn decode_base64(value: &str, name: &str) -> Result<Vec<u8>> {
    let value = strip_wrapping_quotes(value.trim());
    STANDARD
        .decode(value)
        .with_context(|| format!("invalid base64 for --{name}"))
}

fn form_value<'a>(values: &'a [(String, String)], key: &str) -> Option<&'a str> {
    values
        .iter()
        .find_map(|(candidate, value)| (candidate == key).then_some(value.as_str()))
}

fn prompt_line(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .context("failed to read stdin")?;

    Ok(value.trim_end_matches(['\r', '\n']).to_string())
}
