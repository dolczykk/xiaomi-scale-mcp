use std::env;
use std::fs;

use anyhow::{Context, Result, bail};
use xiaomi_client::Client;
use xiaomi_client::auth::LoginChallenge;
use xiaomi_client::errors::XiaomiError;
use xiaomi_client::home::account::GetWeightAccountsRequest;
use xiaomi_client::home::devices::GetDevicesRequest;
use xiaomi_client::home::weight::{WeightIndexInfoRequest, WeightUserDataRequest};

use crate::prompt::{
    line_with_default, required_line, required_number, required_secret, select_index,
};

pub struct App {
    client: Client,
}

impl App {
    pub fn new() -> Result<Self> {
        let region = line_with_default("Xiaomi region", "cn")?;
        let client = Client::new()
            .context("failed to initialize Xiaomi client")?
            .with_region(region);

        Ok(Self { client })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.authenticate().await?;
        println!("Xiaomi Home login succeeded.");

        self.run_home_flow().await
    }

    async fn authenticate(&mut self) -> Result<()> {
        println!("Select Xiaomi Home login method:");
        println!("  1) token");
        println!("  2) username/password");

        match required_line("Login method")?.to_lowercase().as_str() {
            "1" | "token" => {
                let token = required_secret("Xiaomi token")?;
                self.client.login_with_token(&token).await?;
            }
            "2" | "password" | "username/password" => {
                let username = required_line("Xiaomi username")?;
                let password = required_secret("Xiaomi password")?;

                match self.client.login(&username, &password).await {
                    Ok(()) => {}
                    Err(XiaomiError::LoginChallenge(challenge)) => {
                        self.handle_login_challenge(challenge).await?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            value => bail!("unknown login method: {value}"),
        }

        Ok(())
    }

    async fn run_home_flow(&self) -> Result<()> {
        let devices = self
            .client
            .get_devices(&GetDevicesRequest::default())
            .await?;
        println!("Devices:\n{devices:#?}");

        let scales = devices
            .result
            .list
            .iter()
            .filter(|device| device.model.starts_with("yunmai.scales."))
            .collect::<Vec<_>>();
        if scales.is_empty() {
            bail!("no yunmai.scales.* device found");
        }

        println!("Available scales:");
        for (index, scale) in scales.iter().enumerate() {
            println!(
                "  {}) {} ({}, {})",
                index + 1,
                scale.name,
                scale.model,
                scale.device_id
            );
        }
        let scale = scales[select_index("Scale number", scales.len())?];

        let account_request =
            GetWeightAccountsRequest::new(scale.user_id.to_string(), scale.device_id.clone());
        let accounts = self
            .client
            .get_weight_accounts(&account_request, &scale.model)
            .await?;
        println!("Weight accounts:\n{accounts:#?}");

        if accounts.result.is_empty() {
            bail!("selected scale has no weight accounts");
        }

        println!("Available accounts:");
        for (index, account) in accounts.result.iter().enumerate() {
            println!("  {}) {} ({})", index + 1, account.name, account.account_id);
        }
        let account_index = select_index("Account number", accounts.result.len())?;
        let account = &accounts.result[account_index];

        let index_request = WeightIndexInfoRequest::new(
            scale.user_id.to_string(),
            account.account_id.clone(),
            scale.device_id.clone(),
        );
        let index = self
            .client
            .get_weight_index_info(&index_request, &scale.model)
            .await?;
        println!("Weight index information:\n{index:#?}");

        let begin_time = required_number::<i64>("beginTime")?;
        let end_time = required_number::<i64>("endTime")?;
        let page_size = required_number::<u32>("pageSize")?;
        if page_size == 0 {
            bail!("pageSize must be greater than zero");
        }

        let data_request = WeightUserDataRequest::new(
            scale.model.clone(),
            scale.user_id.to_string(),
            scale.device_id.clone(),
            account.account_id.clone(),
            begin_time,
            end_time,
            page_size,
        );
        let data = self.client.get_weight_user_data(&data_request).await?;
        println!("Weight user data:\n{data:#?}");

        Ok(())
    }

    async fn handle_login_challenge(&mut self, mut challenge: LoginChallenge) -> Result<()> {
        loop {
            if let Some(captcha) = &challenge.captcha {
                let captcha_path = env::current_dir()
                    .context("failed to resolve current directory")?
                    .join("xiaomi-captcha.jpg");
                fs::write(&captcha_path, captcha).with_context(|| {
                    format!(
                        "failed to write captcha image to {}",
                        captcha_path.display()
                    )
                })?;
                println!(
                    "Xiaomi captcha required. Image saved to {}.",
                    captcha_path.display()
                );

                let captcha_code = required_line("Xiaomi captcha code")?;
                match self.client.login_with_captcha(&captcha_code).await {
                    Ok(()) => return Ok(()),
                    Err(XiaomiError::LoginChallenge(next)) => {
                        challenge = next;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
            }

            if challenge.verify_phone.is_some() || challenge.verify_email.is_some() {
                if let Some(phone) = &challenge.verify_phone {
                    println!("Xiaomi sent a verification code to phone {phone}.");
                }
                if let Some(email) = &challenge.verify_email {
                    println!("Xiaomi sent a verification code to email {email}.");
                }

                let ticket = required_secret("Xiaomi verification code")?;
                match self.client.login_with_verify(&ticket).await {
                    Ok(()) => return Ok(()),
                    Err(XiaomiError::LoginChallenge(next)) => {
                        challenge = next;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
            }

            return Err(XiaomiError::LoginChallenge(challenge).into());
        }
    }
}
