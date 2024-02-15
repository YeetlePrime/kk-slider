use std::{
    io::{stdout, Write},
    time::Instant,
};

use futures::{stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{fs::{self, File}, io::AsyncWriteExt};

const CONCURRENT_DOWNLOADS: usize = 100;
pub struct Downloader {
    client: Client,
    base_url: String,
    songlist_path: String,
}

// ----- CONSTRUCTORS ---------------------------------------------------------------------------------
impl Downloader {
    pub fn new() -> Downloader {
        Downloader {
            client: Client::new(),
            base_url: "https://nookipedia.com".to_string(),
            songlist_path: "/wiki/List_of_K.K._Slider_songs".to_string(),
        }
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

impl Downloader {
    pub async fn download(&self, directory: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(directory).await?;

        print!("Retrieving urls for all songs... ");
        stdout().flush()?;
        let begin = Instant::now();
        let song_wiki_urls = self.get_song_wiki_urls().await?;
        println!("done! ({} ms)", begin.elapsed().as_millis());

        print!("Song infos for all songs... ");
        stdout().flush()?;
        let begin = Instant::now();
        let song_infos = self
            .get_all_song_infos(&song_wiki_urls)
            .await?
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();
        println!("done! ({} ms)", begin.elapsed().as_millis());


        print!("Downloading songs... ");
        stdout().flush()?;
        let begin = Instant::now();
        self.download_all_songs(&song_infos, directory).await?;
        println!("done! ({} ms)", begin.elapsed().as_millis());

        Ok(())
    }

    async fn download_all_songs(
        &self,
        song_infos: &Vec<SongInfo>,
        directory: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        stream::iter(song_infos)
            .map(|song_info| async { self.download_song(song_info, directory).await })
            .buffer_unordered(CONCURRENT_DOWNLOADS).collect::<Vec<Result<(), Box<dyn std::error::Error>>>>().await;

        Ok(())
    }

    async fn download_song(
        &self,
        song_info: &SongInfo,
        directory: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if song_info.live_song_file_url.is_none() && song_info.aircheck_song_file_url.is_none() {
            return Ok(());
        }

        let directory = format!("{}/{}", directory, song_info.filelized_title()); 
        fs::create_dir_all(&directory).await?;

        if let Some(live_url) = &song_info.live_song_file_url {
            let mut file = File::create(format!("{}/live.flac", directory)).await?;
            let mut stream = self.client.get(live_url).send().await?.bytes_stream();

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        }

        if let Some(aircheck_url) = &song_info.aircheck_song_file_url {
            let mut file = File::create(format!("{}/aircheck.flac", &directory)).await?;
            let mut stream = self.client.get(aircheck_url).send().await?.bytes_stream();

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        }

        Ok(())
    }

}

impl Downloader {
    pub async fn get_all_song_infos(
        &self,
        song_wiki_urls: &[String],
    ) -> Result<Vec<Result<SongInfo, Box<dyn std::error::Error>>>, Box<dyn std::error::Error>> {
        let res = stream::iter(song_wiki_urls)
            .map(|url| async { self.get_song_info(url).await })
            .buffered(CONCURRENT_DOWNLOADS);

        Ok(res
            .collect::<Vec<Result<SongInfo, Box<dyn std::error::Error>>>>()
            .await)
    }

    pub async fn get_song_wiki_urls(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let document = self
            .client
            .get(format!("{}{}", self.base_url, self.songlist_path))
            .send()
            .await?
            .text()
            .await?;

        let html = scraper::Html::parse_document(&document);

        let selector =
            scraper::Selector::parse("table.styled > tbody > tr > td > a[href^=\"/wiki\"][title]")
                .expect("Hard-coded selector is valid");

        Ok(html
            .select(&selector)
            .map(|e| format!("{}{}", self.base_url, e.attr("href").unwrap()))
            .collect())
    }

    pub async fn get_song_info(
        &self,
        song_wiki_url: &str,
    ) -> Result<SongInfo, Box<dyn std::error::Error>> {
        let document = self.client.get(song_wiki_url).send().await?.text().await?;

        let html = scraper::Html::parse_document(&document);

        let number_selector = scraper::Selector::parse("table.infobox > tbody tbody big > i > b")
            .expect("Hard-coded selector is valid");
        let number = html
            .select(&number_selector)
            .map(|e| e.inner_html())
            .next()
            .ok_or("Number must exist in document")?;

        let title_selector =
            scraper::Selector::parse("table.infobox > tbody tbody > tr > th > span")
                .expect("Hard-coded selector is valid");
        let title = html
            .select(&title_selector)
            .map(|e| e.inner_html())
            .next()
            .ok_or("Title must exist in document")?;

        let wiki_url = song_wiki_url.to_string();

        let image_url_selector =
            scraper::Selector::parse("table.infobox > tbody > tr > td > a > img[src]")
                .expect("Hard-coded selector is valid");
        let image_url = html
            .select(&image_url_selector)
            .map(|e| e.attr("src").unwrap().to_string())
            .next();

        let song_file_urls_selector =
            scraper::Selector::parse("table.infobox > tbody > tr > td > audio[src]")
                .expect("Hard-coded selector is valid");
        let song_file_urls: Vec<String> = html
            .select(&song_file_urls_selector)
            .map(|e| e.attr("src").unwrap().to_string())
            .collect();

        let live_song_file_url = song_file_urls
            .iter()
            .find(|s| s.contains("Live"))
            .map(|s| s.to_owned());
        let aircheck_song_file_url = song_file_urls
            .iter()
            .find(|s| s.contains("Aircheck"))
            .map(|s| s.to_owned());

        Ok(SongInfo {
            number,
            title,
            wiki_url,
            image_url,
            live_song_file_url,
            aircheck_song_file_url,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SongInfo {
    pub number: String,
    pub title: String,
    pub wiki_url: String,
    pub image_url: Option<String>,
    pub live_song_file_url: Option<String>,
    pub aircheck_song_file_url: Option<String>,
}

impl SongInfo {
    pub fn filelized_title(&self) -> String {
        self.title.to_lowercase().replace(' ', "_").replace('.', "")
    }
}
