use tokio::sync::OnceCell;

use crate::dal::CacheDal;
use crate::dal::repositories::WeightRepository;

pub struct State {
    cache: CacheDal,
    repository: OnceCell<WeightRepository>,
}

impl State {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            cache: CacheDal::open().await?,
            repository: OnceCell::new(),
        })
    }

    pub async fn repository(&self) -> anyhow::Result<&WeightRepository> {
        self.repository
            .get_or_try_init(|| async { WeightRepository::from_environment(self.cache.clone()) })
            .await
    }
}
