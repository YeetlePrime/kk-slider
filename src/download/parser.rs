use std::{collections::HashMap, slice::Iter};

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::errors::Error;

#[derive(PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize, Debug)]
pub enum SongType {
    Live,
    Aircheck,
    AircheckCheap,
    AircheckRetro,
    AircheckPhono,
    MusicBox,
    DjKkRemix,
}

impl SongType {
    pub fn iterator() -> Iter<'static, SongType> {
        static SONG_TYPES: [SongType; 7] = [
            SongType::Live,
            SongType::Aircheck,
            SongType::AircheckCheap,
            SongType::AircheckRetro,
            SongType::AircheckPhono,
            SongType::MusicBox,
            SongType::DjKkRemix,
        ];

        SONG_TYPES.iter()
    }

    pub fn file_string(&self) -> &'static str {
        match self {
            SongType::Live => "live",
            SongType::Aircheck => "aircheck",
            SongType::AircheckCheap => "aircheck_cheap",
            SongType::AircheckRetro => "aircheck_retro",
            SongType::AircheckPhono => "aircheck_phono",
            SongType::MusicBox => "music_box",
            SongType::DjKkRemix => "dj_kk_remix",
        }
    }

    pub fn url_ending(&self) -> &'static str {
        match self {
            SongType::Live => "%28Live%29.flac",
            SongType::Aircheck => "%28Aircheck%2C_Hi-Fi%29.flac",
            SongType::AircheckCheap => "%28Aircheck%2C_Cheap%29.flac",
            SongType::AircheckRetro => "%28Aircheck%2C_Retro%29.flac",
            SongType::AircheckPhono => "%28Aircheck%2C_Phono%29.flac",
            SongType::MusicBox => "%28Music_Box%29.flac",
            SongType::DjKkRemix => "%28DJ_KK_Remix%29.flac",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SongInfo {
    pub title: String,
    pub number: i32,
    pub wiki_url: String,
    pub image_url: String,
    pub song_file_urls: HashMap<SongType, String>,
}

// ----- CONSTRUCTORS -------------------------------------------------------------------------------------
impl SongInfo {
    pub fn parse_document(document: &str) -> Result<SongInfo, Error> {
        let html = Html::parse_document(document);

        let title = SongInfo::parse_meta_property(&html, "title")
            .ok_or(Error::MissingElement("title".to_string()))?;
        let wiki_url = SongInfo::parse_meta_property(&html, "url")
            .ok_or(Error::MissingElement("url".to_string()))?;
        let image_url = SongInfo::parse_meta_property(&html, "image")
            .ok_or(Error::MissingElement("image".to_string()))?;

        let number_selector = Selector::parse("table.infobox > tbody table big > i > b")
            .expect("Hard-coded selector is valid.");
        let number_string = html
            .select(&number_selector)
            .next()
            .ok_or(Error::MissingElement("number".to_string()))?
            .inner_html();
        let number = number_string[1..]
            .parse::<i32>()
            .map_err(|_| Error::CouldNotParseNumber(number_string))?;

        let song_file_urls = SongInfo::parse_all_song_file_urls(&html);

        Ok(SongInfo {
            title: title.to_string(),
            number,
            wiki_url: wiki_url.to_string(),
            image_url: image_url.to_string(),
            song_file_urls,
        })
    }
}

// ----- PUBLIC METHODS ------------------------------------------------------------
impl SongInfo {
    pub fn filelized_title(&self) -> String {
        self.title.to_lowercase().replace(' ', "_").replace('.', "")
    }
}

// ----- PRIVATE HELPERS ------------------------------------------------------------
impl SongInfo {
    fn parse_meta_property<'a>(html: &'a Html, property: &'a str) -> Option<&'a str> {
        let selector_string = format!("head > meta[property=\"og:{property}\"][content]");
        let selector = Selector::parse(&selector_string).expect("Selector is valid");

        html.select(&selector).next()?.attr("content")
    }

    fn parse_all_song_file_urls(html: &Html) -> HashMap<SongType, String> {
        let mut song_urls = HashMap::new();

        for song_type in SongType::iterator() {
            if let Some(url) = SongInfo::parse_song_file_url(html, song_type) {
                song_urls.insert(song_type.to_owned(), url.to_string());
            }
        }

        song_urls
    }

    fn parse_song_file_url<'a>(html: &'a Html, song_type: &SongType) -> Option<&'a str> {
        // Try finding the song in the infobox table
        // This table usually contains the files for the Live and Aircheck Version
        let selector = Selector::parse(&format!(
            "table.infobox > tbody > tr > td > audio[src$=\"{}\"]",
            song_type.url_ending()
        ))
        .expect("Selector is valid");

        if let Some(infobox_song_url) = html
            .select(&selector)
            .map(|e| e.attr("src"))
            .next()
            .flatten()
        {
            return Some(infobox_song_url);
        }

        // Find the other files in the Music section
        let selector = Selector::parse(&format!(
            "div.tabletop.color-music table > tbody > tr > td > audio[src$=\"{}\"]",
            song_type.url_ending()
        ))
        .expect("Selector is valid");

        html.select(&selector)
            .map(|e| e.attr("src"))
            .next()
            .flatten()
    }
}

#[cfg(test)]
mod tests;
