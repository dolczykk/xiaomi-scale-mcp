use std::sync::Arc;

use crate::cache::CacheStore;
use crate::config::Config;
use crate::credentials::CredentialStore;
use crate::session::XiaomiSession;
use crate::weights::WeightService;

pub(crate) struct App {
    weights: WeightService,
    xiaomi_session: Arc<XiaomiSession>,
}

impl App {
    pub(crate) async fn new(
        config: &Config,
        credentials: Arc<dyn CredentialStore>,
    ) -> anyhow::Result<Self> {
        let xiaomi_session = Arc::new(XiaomiSession::new(config.xiaomi.clone(), credentials));
        let cache = CacheStore::open().await?;
        let weights = WeightService::new(cache, Arc::clone(&xiaomi_session));

        Ok(Self {
            weights,
            xiaomi_session,
        })
    }

    pub(crate) fn weights(&self) -> WeightService {
        self.weights.clone()
    }

    pub(crate) fn xiaomi_session(&self) -> Arc<XiaomiSession> {
        Arc::clone(&self.xiaomi_session)
    }
}
