use std::sync::Mutex;

use crate::credentials::{CredentialStore, validate_token};

#[derive(Default)]
pub(crate) struct MemoryCredentialStore(Mutex<Option<String>>);

impl CredentialStore for MemoryCredentialStore {
    fn load_token(&self) -> anyhow::Result<Option<String>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save_token(&self, token: &str) -> anyhow::Result<()> {
        validate_token(token)?;
        *self.0.lock().unwrap() = Some(token.to_owned());

        Ok(())
    }

    fn delete_token(&self) -> anyhow::Result<bool> {
        Ok(self.0.lock().unwrap().take().is_some())
    }
}
