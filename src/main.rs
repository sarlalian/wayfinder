use anyhow::Result;
use wayfinder::cli::App;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::from_args().await?;
    let args = wayfinder::cli::Args::parse_args();

    app.run(args).await?;

    Ok(())
}
