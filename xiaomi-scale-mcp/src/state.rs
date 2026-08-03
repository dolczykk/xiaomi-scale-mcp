use std::env;

use anyhow::{Context, bail};
use tokio::sync::OnceCell;
use xiaomi_client::Client;
use xiaomi_client::home::account::{GetWeightAccountsRequest, WeightAccount};
use xiaomi_client::home::devices::{DeviceItem, GetDevicesRequest};

use crate::utils::profile_id;

#[derive(Debug)]
pub struct State {
    authenticated: OnceCell<AuthenticatedState>,
}

#[derive(Debug)]
pub struct AuthenticatedState {
    pub client: Client,
    pub profiles: Vec<WeightProfileContext>,
}

#[derive(Debug, Clone)]
pub struct WeightProfileContext {
    pub profile_id: String,
    pub account_id: String,
    pub user_id: String,
    pub device_id: String,
    pub scale_name: String,
    pub scale_model: String,
    pub name: String,
    pub height: String,
    pub weight_target: String,
    pub last_weight_update_time: i64,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        Self {
            authenticated: OnceCell::new(),
        }
    }

    pub async fn authenticated(&self) -> anyhow::Result<&AuthenticatedState> {
        self.authenticated
            .get_or_try_init(|| async { Self::authenticate_and_discover().await })
            .await
    }

    async fn authenticate_and_discover() -> anyhow::Result<AuthenticatedState> {
        let token = env::var("XIAOMI_TOKEN").context("XIAOMI_TOKEN is not set")?;
        let token = token.trim();
        if token.is_empty() {
            bail!("XIAOMI_TOKEN is empty");
        }

        let mut client = Client::new().context("failed to initialize Xiaomi client")?;
        if let Ok(sid) = env::var("XIAOMI_SID") {
            client = client.with_sid(sid);
        }
        if let Ok(device_id) = env::var("XIAOMI_DEVICE_ID") {
            client = client.with_device_id(device_id);
        }
        if let Ok(region) = env::var("XIAOMI_REGION") {
            client = client.with_region(region);
        }

        log::info!("Authenticating with Xiaomi token");
        client
            .login_with_token(token)
            .await
            .context("Xiaomi token authentication failed")?;

        let devices = client
            .get_devices(&GetDevicesRequest::default())
            .await
            .context("failed to discover Xiaomi Home devices")?;
        let mut profiles = Vec::new();

        for scale in devices
            .result
            .list
            .into_iter()
            .filter(|device| device.model.starts_with("yunmai.scales."))
        {
            let account_request =
                GetWeightAccountsRequest::new(scale.user_id.to_string(), scale.device_id.clone());
            let accounts = client
                .get_weight_accounts(&account_request, &scale.model)
                .await
                .with_context(|| {
                    let mut message = String::new();
                    message.push_str("failed to discover weight profiles for scale ");
                    message.push_str(&scale.name);
                    message
                })?;

            profiles.extend(
                accounts
                    .result
                    .into_iter()
                    .map(|account| map_weight_profile(&scale, account)),
            );
        }

        log::info!("Discovered {} Xiaomi weight profiles", profiles.len());
        let state = AuthenticatedState { client, profiles };

        Ok(state)
    }
}

fn map_weight_profile(scale: &DeviceItem, account: WeightAccount) -> WeightProfileContext {
    WeightProfileContext {
        profile_id: profile_id(&scale.device_id, &account.account_id),
        account_id: account.account_id,
        user_id: scale.user_id.to_string(),
        device_id: scale.device_id.clone(),
        scale_name: scale.name.clone(),
        scale_model: scale.model.clone(),
        name: account.name,
        height: account.height,
        weight_target: account.weight_target,
        last_weight_update_time: account.weight_update_time,
    }
}

impl AuthenticatedState {
    pub fn profile(&self, profile_id: &str) -> Option<&WeightProfileContext> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    }
}
