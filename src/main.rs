use std::{fs::File, io::{stdout, Write}, time::Instant};

use kk_slider::{Downloader, SongInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let downloader = Downloader::new();

    let begin = Instant::now();
    print!("Downloading song urls... ");
    stdout().flush()?;
    let song_wiki_urls = downloader.get_song_wiki_urls().await?;
    println!("done!({} ms)", begin.elapsed().as_millis());


    let begin = Instant::now();
    print!("Downloading song infos... ");
    stdout().flush()?;
    let song_infos: Vec<SongInfo> = downloader
        .get_all_song_infos(&song_wiki_urls)
        .await?
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();
    println!("done!({} ms)", begin.elapsed().as_millis());

    let begin = Instant::now();
    print!("Writing json... ");
    stdout().flush()?;
    let mut file = File::create("song_infos.json")?;
    let json = serde_json::to_string_pretty(&song_infos)?;
    file.write_all(json.as_bytes())?;
    println!("done!({} ms)", begin.elapsed().as_millis());


    println!("Downloaded {} song infos!", song_infos.len());

    Ok(())
}
