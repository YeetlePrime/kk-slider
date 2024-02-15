use kk_slider::Downloader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let downloader = Downloader::new();

    downloader.download("songs").await?;

    Ok(())
}
