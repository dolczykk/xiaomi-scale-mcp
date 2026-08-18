use std::sync::Arc;

use anyhow::Context;
use tokio::sync::{Mutex, OnceCell};
use xiaomi_client::Client;
use zeroize::Zeroizing;

use crate::config::XiaomiConfig;
use crate::credentials::{CredentialStore, validate_token};

pub(crate) struct XiaomiSession {
    config: XiaomiConfig,
    credentials: Arc<dyn CredentialStore>,
    state: Mutex<SessionState>,
}

#[derive(Default)]
struct SessionState {
    generation: u64,
    authenticated: Option<Arc<AuthenticatedXiaomi>>,
}

pub(crate) struct AuthenticatedXiaomi {
    user_id: String,
    token: Zeroizing<String>,
    config: XiaomiConfig,
    client: OnceCell<Client>,
}

impl XiaomiSession {
    pub(crate) fn new(config: XiaomiConfig, credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            config,
            credentials,
            state: Mutex::new(SessionState::default()),
        }
    }

    pub(crate) fn login_client(&self) -> anyhow::Result<Client> {
        self.config.client()
    }

    pub(crate) async fn authenticated(&self) -> anyhow::Result<Arc<AuthenticatedXiaomi>> {
        loop {
            let generation = {
                let state = self.state.lock().await;
                if let Some(session) = state.authenticated.as_ref() {
                    return Ok(Arc::clone(session));
                }

                state.generation
            };

            let token = self
                .load_token()
                .await?
                .context("Xiaomi account is not authorized; enter auth in the server console")?;
            let session = Arc::new(AuthenticatedXiaomi::from_token(self.config.clone(), token)?);

            let mut state = self.state.lock().await;
            if state.generation == generation {
                state.authenticated = Some(Arc::clone(&session));
                return Ok(session);
            }
        }
    }

    pub(crate) async fn store_token(&self, token: &str) -> anyhow::Result<()> {
        let credentials = Arc::clone(&self.credentials);
        let token = Zeroizing::new(token.to_owned());
        tokio::task::spawn_blocking(move || credentials.save_token(&token))
            .await
            .context("Xiaomi credential save task failed")??;
        self.invalidate().await;

        Ok(())
    }

    pub(crate) async fn has_token(&self) -> anyhow::Result<bool> {
        let credentials = Arc::clone(&self.credentials);
        tokio::task::spawn_blocking(move || credentials.has_token())
            .await
            .context("Xiaomi credential status task failed")?
    }

    pub(crate) async fn logout(&self) -> anyhow::Result<bool> {
        let credentials = Arc::clone(&self.credentials);
        let deleted = tokio::task::spawn_blocking(move || credentials.delete_token())
            .await
            .context("Xiaomi credential delete task failed")??;
        self.invalidate().await;

        Ok(deleted)
    }

    async fn invalidate(&self) {
        let mut state = self.state.lock().await;
        state.generation = state.generation.wrapping_add(1);
        state.authenticated = None;
    }

    async fn load_token(&self) -> anyhow::Result<Option<String>> {
        let credentials = Arc::clone(&self.credentials);
        tokio::task::spawn_blocking(move || credentials.load_token())
            .await
            .context("Xiaomi credential load task failed")?
    }
}

impl AuthenticatedXiaomi {
    fn from_token(config: XiaomiConfig, token: String) -> anyhow::Result<Self> {
        let token = Zeroizing::new(token);
        let token = token.trim();
        validate_token(token)?;
        let (user_id, pass_token) = token
            .split_once(':')
            .context("validated Xiaomi token is missing a separator")?;
        debug_assert!(!pass_token.is_empty());

        Ok(Self {
            user_id: user_id.to_owned(),
            token: Zeroizing::new(token.to_owned()),
            config,
            client: OnceCell::new(),
        })
    }

    pub(crate) fn user_id(&self) -> &str {
        &self.user_id
    }

    pub(crate) async fn client(&self) -> anyhow::Result<&Client> {
        self.client
            .get_or_try_init(|| async {
                let mut client = self.config.client()?;
                log::info!("Authenticating with Xiaomi token");
                client
                    .login_with_token(&self.token)
                    .await
                    .context("Xiaomi token authentication failed")?;
                Ok(client)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) fn client_is_initialized(&self) -> bool {
        self.client.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;

    use super::XiaomiSession;
    use crate::config::XiaomiConfig;
    use crate::credentials::{CredentialStore, validate_token};

    #[derive(Default)]
    struct MemoryCredentialStore(StdMutex<Option<String>>);

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

    struct BlockingCredentialStore {
        token: StdMutex<Option<String>>,
        load_started: Sender<()>,
        release_load: StdMutex<Option<Receiver<()>>>,
        block_first_load: AtomicBool,
    }

    impl BlockingCredentialStore {
        fn new(token: &str, load_started: Sender<()>, release_load: Receiver<()>) -> Self {
            Self {
                token: StdMutex::new(Some(token.to_owned())),
                load_started,
                release_load: StdMutex::new(Some(release_load)),
                block_first_load: AtomicBool::new(true),
            }
        }
    }

    impl CredentialStore for BlockingCredentialStore {
        fn load_token(&self) -> anyhow::Result<Option<String>> {
            let token = self.token.lock().unwrap().clone();
            if self.block_first_load.swap(false, Ordering::SeqCst) {
                self.load_started.send(()).map_err(|error| {
                    anyhow::anyhow!("failed to signal credential load: {error}")
                })?;
                self.release_load
                    .lock()
                    .unwrap()
                    .take()
                    .expect("first credential load receiver is available")
                    .recv()
                    .map_err(|error| {
                        anyhow::anyhow!("failed to release credential load: {error}")
                    })?;
            }

            Ok(token)
        }

        fn save_token(&self, token: &str) -> anyhow::Result<()> {
            validate_token(token)?;
            *self.token.lock().unwrap() = Some(token.to_owned());
            Ok(())
        }

        fn delete_token(&self) -> anyhow::Result<bool> {
            Ok(self.token.lock().unwrap().take().is_some())
        }
    }

    async fn wait_for_credential_load(load_started: Receiver<()>) {
        tokio::task::spawn_blocking(move || load_started.recv_timeout(Duration::from_secs(1)))
            .await
            .unwrap()
            .expect("credential load did not start");
    }

    #[tokio::test]
    async fn replacing_or_deleting_a_token_invalidates_the_cached_session() {
        let credentials = Arc::new(MemoryCredentialStore::default());
        let session = XiaomiSession::new(XiaomiConfig::default(), credentials.clone());

        session.store_token("user-1:token-1").await.unwrap();
        let first = session.authenticated().await.unwrap();
        session.store_token("user-2:token-2").await.unwrap();
        let replacement = session.authenticated().await.unwrap();
        assert!(!Arc::ptr_eq(&first, &replacement));
        assert_eq!(replacement.user_id(), "user-2");

        assert!(session.logout().await.unwrap());
        assert!(session.authenticated().await.is_err());
    }

    #[tokio::test]
    async fn authorization_update_discards_an_in_flight_session_initialization() {
        let (load_started_sender, load_started_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let credentials = Arc::new(BlockingCredentialStore::new(
            "user-1:token-1",
            load_started_sender,
            release_receiver,
        ));
        let session = Arc::new(XiaomiSession::new(XiaomiConfig::default(), credentials));
        let initializing = {
            let session = Arc::clone(&session);
            tokio::spawn(async move { session.authenticated().await })
        };

        wait_for_credential_load(load_started_receiver).await;
        session.store_token("user-2:token-2").await.unwrap();
        release_sender.send(()).unwrap();

        let authenticated = initializing.await.unwrap().unwrap();
        assert_eq!(authenticated.user_id(), "user-2");
    }

    #[tokio::test]
    async fn logout_discards_an_in_flight_session_initialization() {
        let (load_started_sender, load_started_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let credentials = Arc::new(BlockingCredentialStore::new(
            "user-1:token-1",
            load_started_sender,
            release_receiver,
        ));
        let session = Arc::new(XiaomiSession::new(XiaomiConfig::default(), credentials));
        let initializing = {
            let session = Arc::clone(&session);
            tokio::spawn(async move { session.authenticated().await })
        };

        wait_for_credential_load(load_started_receiver).await;
        assert!(session.logout().await.unwrap());
        release_sender.send(()).unwrap();

        let error = match initializing.await.unwrap() {
            Ok(_) => panic!("session initialization unexpectedly succeeded after logout"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("Xiaomi account is not authorized")
        );
    }
}
