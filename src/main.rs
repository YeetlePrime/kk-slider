use std::{fs::File, io::Write};

use kk_slider::{download::parser::SongInfo, Downloader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let downloader = Downloader::new();

    let wiki_urls = downloader.get_song_wiki_urls().await?;
    let song_infos = downloader
        .get_all_song_infos(&wiki_urls)
        .await?
        .into_iter()
        .filter_map(|si| si.ok())
        .collect::<Vec<SongInfo>>();

    let mut file = File::create("song_infos.json")?;

    let json = serde_json::to_string_pretty(&song_infos)?;

    file.write_all(json.as_bytes())?;

    Ok(())
}
