use crate::{
    download::parser::{SongInfo, SongType},
    errors::Error,
};
use futures::{stream, StreamExt};
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use tracing::{debug, error, info, warn};

const CONCURRENT_DOWNLOADS: usize = 50;

#[derive(Debug)]
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
    pub async fn download(&self, directory: &str) -> Result<(), Error> {
        match fs::create_dir_all(directory).await {
            Ok(_) => {}
            Err(e) => {
                error!("Could not create the directory");
                return Err(Error::FileError(e));
            }
        };

        info!("Retrieving urls");
        let song_wiki_urls = self.get_song_wiki_urls().await?;
        info!(
            "Successfully retrieved urls for {} songs",
            song_wiki_urls.len()
        );

        info!("Loading song infos for all songs");
        let song_infos: Vec<SongInfo> = self
            .get_all_song_infos(&song_wiki_urls)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();
        info!(
            "Successfully retrieved song infos for {} songs",
            song_infos.len()
        );

        let mut file = File::create(format!("{}/song_infos.json", directory)).await?;
        let json = serde_json::to_string_pretty(&song_infos)?;
        file.write_all(json.as_bytes()).await?;

        info!("Starting to download all songs");
        self.download_all_songs(&song_infos, directory).await?;
        info!("Finished downloading all songs");

        Ok(())
    }

    async fn download_all_songs(
        &self,
        song_infos: &Vec<SongInfo>,
        directory: &str,
    ) -> Result<(), Error> {
        stream::iter(song_infos)
            .map(|song_info| async { self.download_song(song_info, directory).await })
            .buffer_unordered(CONCURRENT_DOWNLOADS)
            .collect::<Vec<Result<(), Error>>>()
            .await;

        Ok(())
    }

    #[tracing::instrument(
        name = "download_song",
        skip(self, song_info, directory),
        fields(
            title = song_info.title
        )
    )]
    async fn download_song(&self, song_info: &SongInfo, directory: &str) -> Result<(), Error> {
        if song_info.song_file_urls.is_empty() {
            warn!("Tried to download songs without any song file urls.");
            return Ok(());
        }

        let directory = format!("{}/{}", directory, song_info.filelized_title());
        fs::create_dir_all(&directory).await?;

        info!("Starting to download live file.");
        if let Some(live_url) = song_info.song_file_urls.get(&SongType::Live) {
            let mut file = match File::create(format!("{}/live.flac", directory)).await {
                Ok(file) => {
                    debug!("Created file");
                    file
                }
                Err(e) => {
                    warn!("Could not create file");
                    return Err(Error::FileError(e));
                }
            };
            let mut stream = match self.client.get(live_url).send().await {
                Ok(response) => response.bytes_stream(),
                Err(e) => {
                    warn!("Request failed");
                    return Err(Error::RequestError(e));
                }
            };

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        warn!("Could not read chunk while downloading live file");
                        return Err(Error::RequestError(e));
                    },
                };
                match file.write_all(&chunk).await {
                    Ok(_) => (),
                    Err(e) => {
                        warn!("Could not write chunk to file");
                        return Err(Error::FileError(e));
                    }
                };
            }
            match file.flush().await {
                Ok(_) => {info!("Finished downloading live song.");},
                Err(e) => {
                    warn!("Could not write the remaining buffer");
                    return Err(Error::FileError(e));
                },
            };
        }

        info!("Starting to download aircheck file.");
        if let Some(aircheck_url) = song_info.song_file_urls.get(&SongType::Aircheck) {
            let mut file = File::create(format!("{}/aircheck.flac", &directory)).await?;
            let mut stream = self.client.get(aircheck_url).send().await?.bytes_stream();

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        }
        info!("Finished downloading aircheck song.");

        Ok(())
    }
}

impl Downloader {
    pub async fn get_all_song_infos(
        &self,
        song_wiki_urls: &[String],
    ) -> Vec<Result<SongInfo, Error>> {
        let res = stream::iter(song_wiki_urls)
            .map(|url| async { self.get_song_info(url).await })
            .buffered(CONCURRENT_DOWNLOADS);

        res.collect::<Vec<Result<SongInfo, Error>>>().await
    }

    pub async fn get_song_wiki_urls(&self) -> Result<Vec<String>, Error> {
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

    #[tracing::instrument]
    pub async fn get_song_info(&self, song_wiki_url: &str) -> Result<SongInfo, Error> {
        let response = match self.client.get(song_wiki_url).send().await {
            Ok(response) => response,
            Err(e) => {
                warn!("Could not send the request.");
                return Err(Error::RequestError(e));
            }
        };

        let document = match response.text().await {
            Ok(document) => document,
            Err(e) => {
                warn!("Could not retrieve the response_body");
                return Err(Error::RequestError(e));
            }
        };

        match SongInfo::parse_document(&document) {
            Ok(res) => return Ok(res),
            Err(e) => {
                warn!("Could not parse the song infos");
                return Err(e);
            }
        };
    }
}
