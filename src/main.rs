use dotenvy::dotenv;
use xiaomi_scale_mcp::app::App;

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    dotenv().ok();
    flexi_logger::Logger::try_with_env_or_str("info")?.start()?;

    let app = App::init();
    app?.authorize().await?;

    Ok(())
}
