use std::time::Duration;

use crate::{
    download::parser::{SongInfo, SongType},
    errors::Error,
};
use futures::{stream, StreamExt};
use reqwest::{Client, Response};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use tracing::{debug, error, info, warn};

const CONCURRENT_DOWNLOADS: usize = 10;
const MAX_TRIES: usize = 3;

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
            client: Client::builder()
                .build()
                .expect("Can build this client"),
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

// ----- PUBLIC METHODS ---------------------------------------------------------------------------------
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
            return Err(Error::MissingUrl(
                "Tried downloading songs without any song file urls.".to_string(),
            ));
        }

        let directory = format!("{}/{}", directory, song_info.filelized_title());
        fs::create_dir_all(&directory).await?;

        let mut error_occured = false;
        for song_type in song_info.song_file_urls.keys() {
            if self
                .download_song_of_type(song_info, song_type, &directory)
                .await
                .is_err()
            {
                error_occured = true;
            }
        }

        if error_occured {
            warn!("Could not download all files for \"{}\"", song_info.title);
            return Err(Error::MissingUrl(
                "Could not download all files".to_string(),
            ));
        }

        Ok(())
    }

    #[tracing::instrument(
        name = "Downloader.download_song_of_type",
        skip(self, song_info, directory),
        fields(title = song_info.title),
    )]
    async fn download_song_of_type(
        &self,
        song_info: &SongInfo,
        song_type: &SongType,
        directory: &str,
    ) -> Result<(), Error> {
        if MAX_TRIES == 0 {
            panic!("MAX_SIZE is expected to be greater than 0 but was 0.");
        }

        let url = match song_info.song_file_urls.get(song_type) {
            Some(url) => url,
            None => {
                return Err(Error::MissingUrl(format!(
                    "{}({:?})",
                    song_info.title, song_type
                )));
            }
        };

        for try_counter in 1..MAX_TRIES {
            let filename = format!("{}/{}.flac", directory, song_type.file_string());

            let mut file = match File::create(&filename).await {
                Ok(file) => {
                    debug!("Created file \"{}\"", filename);
                    file
                }
                Err(e) => {
                    if try_counter == MAX_TRIES {
                        error!("Could not create file \"{}\"", filename);
                        return Err(Error::FileError(e));
                    }
                    warn!("Could not create file \"{}\". Trying again.", filename);
                    continue;
                }
            };

            let mut stream = self.make_request(url).await?.bytes_stream();

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        if try_counter == MAX_TRIES {
                            error!("Could not read chunk while downloading live file");
                            return Err(Error::RequestError(e));
                        }
                        warn!("Could not read chunk while downloading live file. Trying again.");
                        continue;
                    }
                };
                match file.write_all(&chunk).await {
                    Ok(_) => (),
                    Err(e) => {
                        if try_counter == MAX_TRIES {
                            error!("Could not write chunk to file");
                            return Err(Error::FileError(e));
                        }
                        warn!("Could not write chunk to file. Trying again.");
                        continue;
                    }
                };
            }

            match file.flush().await {
                Ok(_) => {
                    info!("Downloaded successfully");
                }
                Err(e) => {
                    if try_counter == MAX_TRIES {
                        error!("Could not write the remaining buffer");
                        return Err(Error::FileError(e));
                    }
                    warn!("Could not write the remaining buffer. Trying again.");
                    continue;
                }
            };
        }

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

    #[tracing::instrument(name = "Downloader.get_song_wiki_urls", skip(self))]
    pub async fn get_song_wiki_urls(&self) -> Result<Vec<String>, Error> {
        let url = format!("{}/{}", self.base_url, self.songlist_path);

        let response = self.make_request(&url).await?;
        let document = match response.text().await {
            Ok(document) => document,
            Err(e) => {
                warn!("Could not get response body.");
                return Err(Error::RequestError(e));
            }
        };

        // TODO: This part may go to the parser module
        let html = scraper::Html::parse_document(&document);

        let selector =
            scraper::Selector::parse("table.styled > tbody > tr > td > a[href^=\"/wiki\"][title]")
                .expect("Hard-coded selector is valid");

        Ok(html
            .select(&selector)
            .map(|e| format!("{}{}", self.base_url, e.attr("href").unwrap()))
            .collect())
    }

    #[tracing::instrument(name = "Downloader.get_song_info", skip(self))]
    pub async fn get_song_info(&self, song_wiki_url: &str) -> Result<SongInfo, Error> {
        let response = self.make_request(song_wiki_url).await?;

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

// ----- PRIVATE HELPERS ---------------------------------------------------------------------------------------------------------
impl Downloader {
    async fn make_request(&self, url: &str) -> Result<Response, Error> {
        for try_counter in 1..=MAX_TRIES {
            let response = match self.client.get(url).send().await {
                Ok(response) => response,
                Err(e) => match try_counter {
                    MAX_TRIES => {
                        error!("Could not resolve request {}!", url);
                        return Err(Error::RequestError(e));
                    }
                    _ => {
                        warn!("Could not resolve request {}. Trying again.", url);
                        continue;
                    }
                },
            };

            if !response.status().is_success() {
                match try_counter {
                    MAX_TRIES => {
                        error!("Could not resolve request {}!", url);
                        return Err(Error::ResponseStatusError(
                            response.status(),
                            url.to_string(),
                        ));
                    }
                    _ => {
                        warn!("Could not resolve request {}. Trying again.", url);
                        continue;
                    }
                }
            }

            return Ok(response);
        }

        panic!("MAX_TRIES is not allowed to be 0")
    }
}
