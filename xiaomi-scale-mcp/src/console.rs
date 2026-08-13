use std::io::{self, Write};
use std::str::FromStr;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use anyhow::{Context, bail};
use tempfile::Builder;
use tokio::runtime::Handle;
use xiaomi_client::Client;
use xiaomi_client::auth::LoginChallenge;
use xiaomi_client::errors::XiaomiError;
use zeroize::Zeroizing;

use crate::config::XiaomiConfig;
use crate::credentials::CredentialStore;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsoleCommand {
    Auth,
    Status,
    Logout,
    Help,
}

impl FromStr for ConsoleCommand {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auth" => Ok(Self::Auth),
            "status" => Ok(Self::Status),
            "logout" => Ok(Self::Logout),
            "help" => Ok(Self::Help),
            "" => Err(String::new()),
            value => Err(format!("unknown command: {value}")),
        }
    }
}

pub(crate) fn spawn_console(
    runtime: Handle,
    state: Arc<State>,
    credentials: Arc<dyn CredentialStore>,
    xiaomi: XiaomiConfig,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("xiaomi-auth-console".to_string())
        .spawn(move || {
            if let Err(error) = run_console(&runtime, &state, credentials.as_ref(), &xiaomi) {
                log::error!("Xiaomi authentication console stopped: {error:#}");
            }
        })
}

fn run_console(
    runtime: &Handle,
    state: &State,
    credentials: &dyn CredentialStore,
    xiaomi: &XiaomiConfig,
) -> anyhow::Result<()> {
    println!("Xiaomi console ready. Enter help to list commands.");

    loop {
        let command = match prompt_line("xiaomi-scale-mcp") {
            Ok(command) => command,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error).context("failed to read console command"),
        };

        match command.parse::<ConsoleCommand>() {
            Ok(ConsoleCommand::Auth) => {
                if let Err(error) = authenticate(runtime, state, credentials, xiaomi) {
                    eprintln!("Authentication failed: {error:#}");
                }
            }
            Ok(ConsoleCommand::Status) => print_status(credentials),
            Ok(ConsoleCommand::Logout) => logout(runtime, state, credentials)?,
            Ok(ConsoleCommand::Help) => print_help(),
            Err(error) if error.is_empty() => {}
            Err(error) => eprintln!("{error}. Enter help to list commands."),
        }
    }
}

fn authenticate(
    runtime: &Handle,
    state: &State,
    credentials: &dyn CredentialStore,
    xiaomi: &XiaomiConfig,
) -> anyhow::Result<()> {
    let username = required_line("Xiaomi email, phone, or account ID")?;
    let password = Zeroizing::new(
        rpassword::prompt_password("Xiaomi password: ")
            .context("failed to read Xiaomi password")?,
    );
    if password.is_empty() {
        bail!("Xiaomi password is required");
    }

    let token = runtime.block_on(authenticate_client(xiaomi, &username, &password))?;
    credentials.save_token(&token)?;
    runtime.block_on(state.invalidate_repository());

    println!("Xiaomi account authorization succeeded.");
    Ok(())
}

async fn authenticate_client(
    xiaomi: &XiaomiConfig,
    username: &str,
    password: &str,
) -> anyhow::Result<Zeroizing<String>> {
    let mut client = xiaomi.client()?;

    match client.login(username, password).await {
        Ok(()) => {}
        Err(XiaomiError::LoginChallenge(challenge)) => {
            handle_login_challenge(&mut client, challenge).await?;
        }
        Err(error) => return Err(error.into()),
    }

    Ok(Zeroizing::new(client.token()))
}

async fn handle_login_challenge(
    client: &mut Client,
    mut challenge: LoginChallenge,
) -> anyhow::Result<()> {
    loop {
        if let Some(captcha) = &challenge.captcha {
            let mut file = Builder::new()
                .prefix("xiaomi-captcha-")
                .suffix(".jpg")
                .tempfile()
                .context("failed to create temporary captcha file")?;
            file.write_all(captcha)
                .context("failed to write temporary captcha file")?;
            file.flush()
                .context("failed to flush temporary captcha file")?;

            println!(
                "Xiaomi captcha required. Image saved to {}.",
                file.path().display()
            );

            let captcha_code = Zeroizing::new(required_line("Xiaomi captcha code")?);
            match client.login_with_captcha(&captcha_code).await {
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

            let ticket = Zeroizing::new(
                rpassword::prompt_password("Xiaomi verification code: ")
                    .context("failed to read Xiaomi verification code")?,
            );
            if ticket.is_empty() {
                bail!("Xiaomi verification code is required");
            }

            match client.login_with_verify(&ticket).await {
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

fn print_status(credentials: &dyn CredentialStore) {
    match credentials.has_token() {
        Ok(true) => println!("Xiaomi credential is stored."),
        Ok(false) => println!("Xiaomi account is not authorized."),
        Err(error) => eprintln!("Unable to read Xiaomi credential status: {error:#}"),
    }
}

fn logout(
    runtime: &Handle,
    state: &State,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<()> {
    let confirmation = prompt_line("Delete the stored Xiaomi credential? [y/N]")?;
    if !matches!(confirmation.to_ascii_lowercase().as_str(), "y" | "yes") {
        println!("Logout cancelled.");

        return Ok(());
    }

    let deleted = credentials.delete_token()?;
    runtime.block_on(state.invalidate_repository());

    if deleted {
        println!("Stored Xiaomi credential deleted.");
    } else {
        println!("No stored Xiaomi credential was found.");
    }

    Ok(())
}

fn print_help() {
    println!("Available commands:");
    println!("  auth    Authorize a Xiaomi account");
    println!("  status  Check whether a Xiaomi credential is stored");
    println!("  logout  Delete the stored Xiaomi credential");
    println!("  help    Show this command list");
}

fn required_line(label: &str) -> anyhow::Result<String> {
    let value = prompt_line(label)?;
    if value.is_empty() {
        bail!("{label} is required");
    }

    Ok(value)
}

fn prompt_line(label: &str) -> io::Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;

    let mut value = String::new();
    if io::stdin().read_line(&mut value)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "standard input closed",
        ));
    }

    Ok(value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::ConsoleCommand;

    #[test]
    fn parses_supported_commands_case_insensitively() {
        assert_eq!(
            ConsoleCommand::from_str(" AUTH ").unwrap(),
            ConsoleCommand::Auth
        );
        assert_eq!(
            ConsoleCommand::from_str("status").unwrap(),
            ConsoleCommand::Status
        );
        assert_eq!(
            ConsoleCommand::from_str("Logout").unwrap(),
            ConsoleCommand::Logout
        );
        assert_eq!(
            ConsoleCommand::from_str("help").unwrap(),
            ConsoleCommand::Help
        );
    }

    #[test]
    fn rejects_unknown_command() {
        assert_eq!(
            ConsoleCommand::from_str("serve").unwrap_err(),
            "unknown command: serve"
        );
    }
}
