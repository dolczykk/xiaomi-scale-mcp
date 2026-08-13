use anyhow::{Context, bail};
use keyring::{Entry, Error as KeyringError};

const CREDENTIAL_SERVICE: &str = "xiaomi-scale-mcp";
const CREDENTIAL_USERNAME: &str = "xiaomi-home-token";

pub(crate) trait CredentialStore: Send + Sync {
    fn load_token(&self) -> anyhow::Result<Option<String>>;
    fn save_token(&self, token: &str) -> anyhow::Result<()>;
    fn delete_token(&self) -> anyhow::Result<bool>;

    fn has_token(&self) -> anyhow::Result<bool> {
        Ok(self.load_token()?.is_some())
    }
}

#[derive(Debug, Default)]
pub(crate) struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry(&self) -> anyhow::Result<Entry> {
        Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USERNAME)
            .context("failed to access the operating system credential store")
    }
}

impl CredentialStore for SystemCredentialStore {
    fn load_token(&self) -> anyhow::Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(token) => {
                validate_token(&token)?;
                Ok(Some(token))
            }
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(error).context("failed to read the Xiaomi credential"),
        }
    }

    fn save_token(&self, token: &str) -> anyhow::Result<()> {
        validate_token(token)?;
        self.entry()?
            .set_password(token)
            .context("failed to store the Xiaomi credential")
    }

    fn delete_token(&self) -> anyhow::Result<bool> {
        match self.entry()?.delete_credential() {
            Ok(()) => Ok(true),
            Err(KeyringError::NoEntry) => Ok(false),
            Err(error) => Err(error).context("failed to delete the Xiaomi credential"),
        }
    }
}

pub(crate) fn validate_token(token: &str) -> anyhow::Result<()> {
    let token = token.trim();
    let Some((user_id, pass_token)) = token.split_once(':') else {
        bail!("Xiaomi token must use the userId:passToken format");
    };

    if user_id.is_empty() || pass_token.is_empty() {
        bail!("Xiaomi token must contain a non-empty user ID and pass token");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_token;

    #[test]
    fn accepts_valid_xiaomi_token() {
        validate_token("123:pass-token").unwrap();
    }

    #[test]
    fn rejects_invalid_xiaomi_tokens() {
        for token in ["", "123", ":pass-token", "123:"] {
            assert!(validate_token(token).is_err(), "accepted {token:?}");
        }
    }
}
