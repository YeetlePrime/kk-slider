use kk_slider::{Downloader, SongInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let downloader = Downloader::new();

    downloader.download("songs").await?;

    Ok(())
}
