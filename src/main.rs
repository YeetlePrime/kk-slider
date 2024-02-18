use kk_slider::{errors::Error, Downloader};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Vec<Error>> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");

    run().await.unwrap();

    Ok(())
}

async fn run() -> Result<(), Vec<Error>> {
    let downloader = Downloader::new();

    downloader.download("songs").await
}
