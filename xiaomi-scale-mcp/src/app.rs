use anyhow::{Context, bail};
use std::io::Write;
use std::{env, io};
use xiaomi_client::Client;
use xiaomi_client::auth::LoginChallenge;
use xiaomi_client::errors::XiaomiError;

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
        let client = Client::new().with_context(|| "Failed to initialize client".to_string())?;

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

        Ok(())
    }

    async fn handle_login_challenge(
        &mut self,
        mut challenge: LoginChallenge,
    ) -> anyhow::Result<()> {
        loop {
            if challenge.verify_phone.is_some() || challenge.verify_email.is_some() {
                if let Some(phone) = &challenge.verify_phone {
                    bail!("Verification ticket required for phone {phone}.");
                }
                if let Some(email) = &challenge.verify_email {
                    bail!("Verification ticket required for email {email}.");
                }

                let ticket = self.read_verify_ticket(&challenge)?;

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

    fn read_verify_ticket(&mut self, challenge: &LoginChallenge) -> anyhow::Result<String> {
        print!("Enter Xiaomi verification code: ");
        io::stdout().flush().context("failed to flush stdout")?;

        let mut ticket = String::new();
        io::stdin()
            .read_line(&mut ticket)
            .context("failed to read Xiaomi verification code from stdin")?;

        let ticket = ticket.trim().to_string();
        if ticket.is_empty() {
            let error = XiaomiError::LoginChallenge(challenge.clone());

            bail!(error);
        }

        Ok(ticket)
    }
}
