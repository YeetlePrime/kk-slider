use kk_slider::{errors::Error, Downloader};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");

    run().await.unwrap();

    Ok(())
}

async fn run() -> Result<(), Error> {
    let downloader = Downloader::new();

    downloader.download("songs").await?;

    //let wiki_urls = downloader.get_song_wiki_urls().await?;
    //let song_infos = downloader
    //    .get_all_song_infos(&wiki_urls)
    //    .await
    //    .into_iter()
    //    .filter_map(|si| si.ok())
    //    .collect::<Vec<SongInfo>>();
//
    //let mut file = File::create("song_infos.json")?;
//
    //let json = serde_json::to_string_pretty(&song_infos)?;
//
    //file.write_all(json.as_bytes())?;

    Ok(())
}
