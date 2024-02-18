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
            client: Client::builder().build().expect("Can build this client"),
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
    pub async fn download(&self, directory: &str) -> Result<(), Vec<Error>> {
        match fs::create_dir_all(directory).await {
            Ok(_) => {}
            Err(e) => {
                error!("Could not create the directory");
                return Err(vec![Error::FileError(e)]);
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

        let mut file = File::create(format!("{}/song_infos.json", directory))
            .await
            .map_err(|e| vec![Error::FileError(e)])?;
        let json =
            serde_json::to_string_pretty(&song_infos).map_err(|e| vec![Error::JsonError(e)])?;
        file.write_all(json.as_bytes())
            .await
            .map_err(|e| vec![Error::FileError(e)])?;

        info!("Starting to download all songs");
        self.download_all_songs(&song_infos, directory).await?;
        info!("Finished downloading all songs");

        Ok(())
    }

    async fn download_all_songs(
        &self,
        song_infos: &Vec<SongInfo>,
        directory: &str,
    ) -> Result<(), Vec<Error>> {
        stream::iter(song_infos)
            .map(|song_info| async { self.download_song(song_info, directory).await })
            .buffer_unordered(CONCURRENT_DOWNLOADS)
            .collect::<Vec<Result<(), Vec<Error>>>>()
            .await;

        Ok(())
    }

    async fn download_song(&self, song_info: &SongInfo, directory: &str) -> Result<(), Vec<Error>> {
        if song_info.song_file_urls.is_empty() {
            warn!("Tried to download songs without any song file urls.");
            return Err(vec![Error::MissingUrl(
                "Tried downloading songs without any song file urls.".to_string(),
            )]);
        }

        let directory = format!("{}/{}", directory, song_info.filelized_title());
        fs::create_dir_all(&directory)
            .await
            .map_err(|e| vec![Error::FileError(e)])?;

        let mut errors: Vec<Error> = vec![];
        if let Err(mut e) = self.download_image(song_info, &directory).await {
            errors.append(&mut e);
        }

        for song_type in song_info.song_file_urls.keys() {
            match self
                .download_song_of_type(song_info, song_type, &directory)
                .await
            {
                Ok(_) => (),
                Err(mut e) => {
                    errors.append(&mut e);
                },
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }


    #[tracing::instrument(
        name = "download_image",
        skip(self, song_info, directory),
        fields(title = song_info.title),
    )]
    async fn download_image(
        &self,
        song_info: &SongInfo,
        directory: &str,
    ) -> Result<(), Vec<Error>> {

        let file_ending;
        if song_info.image_url.ends_with(".png") {
            file_ending = "png";
        } else if song_info.image_url.ends_with(".jpg") || song_info.image_url.ends_with(".jpeg") {
            file_ending = "jpg;"
        } else {
            warn!("File ending not supported");
            return Err(vec![Error::Error(format!("No valid file ending for {}", song_info.image_url))]);
        }

        let filename = format!("{}/image.{}", directory, file_ending);

        self.download_file(&song_info.image_url, &filename).await
    }


    #[tracing::instrument(
        name = "download_song_of_type",
        skip(self, song_info, directory),
        fields(title = song_info.title),
    )]
    async fn download_song_of_type(
        &self,
        song_info: &SongInfo,
        song_type: &SongType,
        directory: &str,
    ) -> Result<(), Vec<Error>> {
        let url = match song_info.song_file_urls.get(song_type) {
            Some(url) => url,
            None => {
                return Err(vec![Error::MissingUrl(format!(
                    "{}({:?})",
                    song_info.title, song_type
                ))]);
            }
        };

        let filename = format!("{}/{}.flac", directory, song_type.file_string());

        self.download_file(url, &filename).await
    }
}

impl Downloader {
    async fn get_all_song_infos(
        &self,
        song_wiki_urls: &[String],
    ) -> Vec<Result<SongInfo, Vec<Error>>> {
        let res = stream::iter(song_wiki_urls)
            .map(|url| async { self.get_song_info(url).await })
            .buffered(CONCURRENT_DOWNLOADS);

        res.collect().await
    }

    #[tracing::instrument(name = "Downloader.get_song_wiki_urls", skip(self))]
    async fn get_song_wiki_urls(&self) -> Result<Vec<String>, Vec<Error>> {
        let url = format!("{}/{}", self.base_url, self.songlist_path);

        let document = self.get_document(&url).await?;

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
    async fn get_song_info(&self, song_wiki_url: &str) -> Result<SongInfo, Vec<Error>> {
        let document = self.get_document(song_wiki_url).await?;

        match SongInfo::parse_document(&document) {
            Ok(res) => return Ok(res),
            Err(e) => {
                warn!("Could not parse the song infos");
                return Err(vec![e]);
            }
        };
    }
}

// ----- PRIVATE HELPERS ---------------------------------------------------------------------------------------------------------
impl Downloader {
    async fn download_file(&self, url: &str, filename: &str) -> Result<(), Vec<Error>> {
        let mut errors = vec![];

        for _ in 1..=MAX_TRIES {
            match self.try_download_file(url, filename).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    errors.push(e);
                }
            }
        }

        Err(errors)
    }

    async fn try_download_file(&self, url: &str, filename: &str) -> Result<(), Error> {
        let mut file = match File::create(filename).await {
            Ok(file) => {
                debug!("Created file {}", filename);
                file
            }
            Err(e) => {
                warn!("Could not create file");
                return Err(Error::FileError(e));
            }
        };

        let mut stream = self.get(url).await?.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(chunk) => chunk,
                Err(e) => {
                    warn!("Failed to read chunk");
                    drop(file);
                    fs::remove_file(filename).await.unwrap();
                    return Err(Error::RequestError(e));
                }
            };

            match file.write_all(&chunk).await {
                Ok(_) => (),
                Err(e) => {
                    warn!("Failed to write chunk");
                    drop(file);
                    fs::remove_file(filename).await.unwrap();
                    return Err(Error::FileError(e));
                }
            }
        }

        match file.flush().await {
            Ok(_) => {
                info!("Finished downloading");
                Ok(())
            }
            Err(e) => {
                warn!("Could not write remaining buffer");
                drop(file);
                fs::remove_file(filename).await.unwrap();
                Err(Error::FileError(e))
            }
        }
    }

    async fn get_document(&self, url: &str) -> Result<String, Vec<Error>> {
        let mut errors = vec![];

        for _ in 1..=MAX_TRIES {
            match self.try_get_document(url).await {
                Ok(document) => return Ok(document),
                Err(e) => {
                    errors.push(e);
                }
            }
        }

        Err(errors)
    }

    async fn try_get_document(&self, url: &str) -> Result<String, Error> {
        match self.get(url).await?.text().await {
            Ok(document) => Ok(document),
            Err(e) => {
                warn!("Could not get response body");
                Err(Error::RequestError(e))
            }
        }
    }

    async fn get(&self, url: &str) -> Result<Response, Error> {
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(e) => {
                warn!("Could not send request");
                return Err(Error::RequestError(e));
            }
        };

        if response.status().is_success() {
            return Ok(response);
        }

        warn!("Server Error: {}", response.status());
        Err(Error::ResponseStatusError(
            response.status(),
            url.to_string(),
        ))
    }
}
