use std::sync::Arc;

use anyhow::Context;
use tokio::sync::Mutex;

use crate::config::{Config, XiaomiConfig};
use crate::credentials::CredentialStore;
use crate::dal::CacheDal;
use crate::dal::repositories::WeightRepository;

pub struct State {
    cache: CacheDal,
    xiaomi: XiaomiConfig,
    credentials: Arc<dyn CredentialStore>,
    repository: Mutex<Option<Arc<WeightRepository>>>,
}

impl State {
    pub async fn new(
        config: &Config,
        credentials: Arc<dyn CredentialStore>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            cache: CacheDal::open().await?,
            xiaomi: config.xiaomi.clone(),
            credentials,
            repository: Mutex::new(None),
        })
    }

    pub async fn repository(&self) -> anyhow::Result<Arc<WeightRepository>> {
        let mut repository = self.repository.lock().await;
        if let Some(repository) = repository.as_ref() {
            return Ok(Arc::clone(repository));
        }

        let credentials = Arc::clone(&self.credentials);
        let token = tokio::task::spawn_blocking(move || credentials.load_token())
            .await
            .context("Xiaomi credential task failed")??
            .context("Xiaomi account is not authorized; enter auth in the server console")?;
        
        let created = Arc::new(WeightRepository::from_token(
            self.cache.clone(),
            self.xiaomi.clone(),
            token,
        )?);

        *repository = Some(Arc::clone(&created));
        Ok(created)
    }

    pub async fn invalidate_repository(&self) {
        *self.repository.lock().await = None;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex as StdMutex};

    use super::State;
    use crate::config::XiaomiConfig;
    use crate::credentials::{CredentialStore, validate_token};
    use crate::dal::CacheDal;

    #[derive(Default)]
    struct MemoryCredentialStore {
        token: StdMutex<Option<String>>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load_token(&self) -> anyhow::Result<Option<String>> {
            Ok(self.token.lock().unwrap().clone())
        }

        fn save_token(&self, token: &str) -> anyhow::Result<()> {
            validate_token(token)?;
            *self.token.lock().unwrap() = Some(token.to_string());
            Ok(())
        }

        fn delete_token(&self) -> anyhow::Result<bool> {
            Ok(self.token.lock().unwrap().take().is_some())
        }
    }

    async fn state(credentials: Arc<dyn CredentialStore>) -> State {
        State {
            cache: CacheDal::in_memory().await.unwrap(),
            xiaomi: XiaomiConfig::default(),
            credentials,
            repository: Default::default(),
        }
    }

    #[tokio::test]
    async fn missing_credential_returns_actionable_error() {
        let credentials: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
        let state = state(credentials).await;

        let error = match state.repository().await {
            Ok(_) => panic!("repository unexpectedly initialized"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("enter auth in the server console")
        );
    }

    #[tokio::test]
    async fn invalidating_repository_uses_replaced_credential() {
        let credentials = Arc::new(MemoryCredentialStore::default());
        credentials.save_token("user-1:token-1").unwrap();
        let state = state(credentials.clone()).await;

        let first = state.repository().await.unwrap();
        credentials.save_token("user-2:token-2").unwrap();
        let cached = state.repository().await.unwrap();
        assert!(Arc::ptr_eq(&first, &cached));

        state.invalidate_repository().await;
        let replaced = state.repository().await.unwrap();
        assert!(!Arc::ptr_eq(&first, &replaced));
    }

    #[tokio::test]
    async fn deleting_credential_prevents_repository_recreation() {
        let credentials = Arc::new(MemoryCredentialStore::default());
        credentials.save_token("user-1:token-1").unwrap();
        let state = state(credentials.clone()).await;
        state.repository().await.unwrap();

        assert!(credentials.delete_token().unwrap());
        state.invalidate_repository().await;
        assert!(state.repository().await.is_err());
    }
}
