use dotenvy::dotenv;
use xiaomi_scale_mcp::app::App;

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    dotenv().ok();

    let app = App::init();
    app?.authorize().await?;

    Ok(())
}
