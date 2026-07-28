use anyhow::{Context, bail};
use std::fs;
use std::io::Write;
use std::{env, io};
use xiaomi_client::Client;
use xiaomi_client::auth::LoginChallenge;
use xiaomi_client::errors::XiaomiError;
use xiaomi_client::home::account::GetWeightAccountsRequest;
use xiaomi_client::home::devices::GetDevicesRequest;
use xiaomi_client::home::weight::{WeightIndexInfoRequest, WeightUserDataRequest};

#[derive(Debug)]
struct Credentials {
    login: String,
    password: String,
    token: Option<String>,
}

#[derive(Debug)]
pub struct App {
    client: Client,
    credentials: Credentials,
}

impl App {
    fn new(client: Client, credentials: Credentials) -> Self {
        App {
            client,
            credentials,
        }
    }

    pub fn init() -> anyhow::Result<Self, anyhow::Error> {
        let mut client =
            Client::new().with_context(|| "Failed to initialize client".to_string())?;

        if let Ok(sid) = env::var("XIAOMI_SID") {
            client = client.with_sid(sid);
        }
        if let Ok(device_id) = env::var("XIAOMI_DEVICE_ID") {
            client = client.with_device_id(device_id);
        }
        if let Ok(region) = env::var("XIAOMI_REGION") {
            client = client.with_region(region);
        }

        let creds = Credentials {
            login: env::var("XIAOMI_USERNAME").context("XIAOMI_USERNAME not set")?,
            password: env::var("XIAOMI_PASSWORD").context("XIAOMI_PASSWORD not set")?,
            token: env::var("XIAOMI_TOKEN").ok(),
        };

        Ok(App::new(client, creds))
    }

    pub async fn authorize(&mut self) -> anyhow::Result<(), anyhow::Error> {
        if let Some(token) = &mut self.credentials.token {
            log::info!("Xiaomi token login...");
            self.client.login_with_token(token.as_str()).await?;
            log::info!("Token login succeeded");

            self.demo().await?;

            return Ok(());
        }

        log::info!("Testing Xiaomi username/password login...");

        let login_result = self
            .client
            .login(
                self.credentials.login.as_str(),
                self.credentials.password.as_str(),
            )
            .await;
        match login_result {
            Ok(()) => {}
            Err(XiaomiError::LoginChallenge(challenge)) => {
                self.handle_login_challenge(challenge).await?;
            }
            Err(err) => return Err(err.into()),
        }

        log::info!("Login succeeded");
        log::info!("Token: {}", self.client.token());
        log::info!("Device ID: {}", self.client.device_id().unwrap());

        self.demo().await?;

        Ok(())
    }

    async fn demo(&self) -> anyhow::Result<()> {
        let devices_request = GetDevicesRequest::default();
        let response = self.client.get_devices(&devices_request).await?;

        let weight = response
            .result
            .list
            .iter()
            .find(|d| d.model.contains("yunmai.scales.ms104"))
            .unwrap();

        println!("List of devices: {:?}", response);

        let account_request =
            GetWeightAccountsRequest::new(weight.user_id.to_string(), weight.device_id.clone());
        let account_response = self
            .client
            .get_weight_accounts(&account_request, &weight.model)
            .await?;

        println!("accounts: {:?}", account_response);

        let index_request = WeightIndexInfoRequest::new(
            weight.user_id.to_string(),
            account_response.result.first().unwrap().account_id.clone(),
            weight.device_id.clone(),
        );

        let index = self
            .client
            .get_weight_index_info(&index_request, &weight.model)
            .await?;

        println!("Weight index info: {:?}", index);

        let request = WeightUserDataRequest {
            model: weight.model.clone(),
            uid: weight.user_id.to_string(),
            device_id: weight.device_id.clone(),
            account_id: account_response.result.first().unwrap().account_id.clone(),
            begin_time: 1784237018435,
            end_time: -28800,
            page_size: 20,
        };
        let weight_response = self.client.get_weight_user_data(&request).await?;

        println!("Weight data: {:?}", weight_response);

        Ok(())
    }

    async fn handle_login_challenge(
        &mut self,
        mut challenge: LoginChallenge,
    ) -> anyhow::Result<()> {
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

                let captcha_code = self.read_challenge_input(
                    "Enter Xiaomi captcha code: ",
                    &challenge,
                    "failed to read Xiaomi captcha code from stdin",
                )?;

                match self.client.login_with_captcha(&captcha_code).await {
                    Ok(()) => return Ok(()),
                    Err(XiaomiError::LoginChallenge(next)) => {
                        challenge = next;
                        continue;
                    }
                    Err(err) => return Err(err.into()),
                }
            }

            if challenge.verify_phone.is_some() || challenge.verify_email.is_some() {
                if let Some(phone) = &challenge.verify_phone {
                    println!("Xiaomi sent a verification code to phone {phone}.");
                }
                if let Some(email) = &challenge.verify_email {
                    println!("Xiaomi sent a verification code to email {email}.");
                }

                let ticket = self.read_challenge_input(
                    "Enter Xiaomi verification code: ",
                    &challenge,
                    "failed to read Xiaomi verification code from stdin",
                )?;

                match self.client.login_with_verify(&ticket).await {
                    Ok(()) => return Ok(()),
                    Err(XiaomiError::LoginChallenge(next)) => {
                        challenge = next;
                        continue;
                    }
                    Err(err) => return Err(err.into()),
                }
            }

            let error = XiaomiError::LoginChallenge(challenge);

            return Err(error.into());
        }
    }

    fn read_challenge_input(
        &mut self,
        prompt: &str,
        challenge: &LoginChallenge,
        read_error: &str,
    ) -> anyhow::Result<String> {
        print!("{prompt}");
        io::stdout().flush().context("failed to flush stdout")?;

        let mut value = String::new();
        io::stdin()
            .read_line(&mut value)
            .context(read_error.to_string())?;

        let value = value.trim().to_string();
        if value.is_empty() {
            let error = XiaomiError::LoginChallenge(challenge.clone());

            bail!(error);
        }

        Ok(value)
    }
}
