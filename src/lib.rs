use reqwest::Client;
use serde::{Deserialize, Serialize};

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
    pub async fn get_all_song_infos(
        &self,
        song_wiki_urls: &[String],
    ) -> Result<Vec<Result<SongInfo, Box<dyn std::error::Error>>>, Box<dyn std::error::Error>> {
        let futures = song_wiki_urls
            .iter()
            .map(|url| async { self.get_song_info(url).await });

        Ok(futures::future::join_all(futures).await)
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
            .expect("Number must exist in document");

        let title_selector =
            scraper::Selector::parse("table.infobox > tbody tbody > tr > th > span")
                .expect("Hard-coded selector is valid");
        let title = html
            .select(&title_selector)
            .map(|e| e.inner_html())
            .next()
            .expect("Title must exist in document");

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
