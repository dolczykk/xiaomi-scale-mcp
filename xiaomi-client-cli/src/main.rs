mod app;
mod prompt;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version)]
struct Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::parse();

    let mut app = app::App::new()?;
    app.run().await
}
