use tokio::sync::OnceCell;

use crate::config::{Config, XiaomiConfig};
use crate::dal::CacheDal;
use crate::dal::repositories::WeightRepository;

pub struct State {
    cache: CacheDal,
    xiaomi: XiaomiConfig,
    repository: OnceCell<WeightRepository>,
}

impl State {
    pub async fn new(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            cache: CacheDal::open().await?,
            xiaomi: config.xiaomi.clone(),
            repository: OnceCell::new(),
        })
    }

    pub async fn repository(&self) -> anyhow::Result<&WeightRepository> {
        self.repository
            .get_or_try_init(|| async {
                WeightRepository::from_config(self.cache.clone(), self.xiaomi.clone())
            })
            .await
    }
}
