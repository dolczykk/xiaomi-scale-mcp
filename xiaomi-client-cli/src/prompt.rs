use std::fmt::Display;
use std::io::{self, Write};
use std::str::FromStr;

use anyhow::{Context, Result, bail};

pub fn required_line(label: &str) -> Result<String> {
    let value = prompt_line(label)?;
    if value.is_empty() {
        bail!("{label} is required");
    }

    Ok(value)
}

pub fn line_with_default(label: &str, default: &str) -> Result<String> {
    let value = prompt_line(&format!("{label} [{default}]"))?;
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value)
    }
}

pub fn required_secret(label: &str) -> Result<String> {
    let value =
        rpassword::prompt_password(format!("{label}: ")).context("failed to read hidden input")?;
    if value.is_empty() {
        bail!("{label} is required");
    }

    Ok(value)
}

pub fn required_number<T>(label: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    let value = required_line(label)?;
    value
        .parse::<T>()
        .map_err(|error| anyhow::anyhow!("{label} must be a valid number: {error}"))
}

pub fn select_index(label: &str, item_count: usize) -> Result<usize> {
    if item_count == 0 {
        bail!("cannot select from an empty list");
    }
    if item_count == 1 {
        return Ok(0);
    }

    let selection = required_number::<usize>(label)?;
    if !(1..=item_count).contains(&selection) {
        bail!("{label} must be between 1 and {item_count}");
    }

    Ok(selection - 1)
}

fn prompt_line(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .context("failed to read stdin")?;

    Ok(value.trim().to_string())
}
