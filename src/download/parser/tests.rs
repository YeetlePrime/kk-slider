use std::{fs::File, io::Read};

use crate::download::parser::SongType;

use super::SongInfo;

#[test]
fn parse_document_happy_path() {
    // arrange
    let mut file = File::open("src/download/parser/tests/happy_path.html").unwrap();
    let mut document = String::new();
    file.read_to_string(&mut document).unwrap();

    // act
    let song_info = SongInfo::parse_document(&document).unwrap();

    // assert
    assert_eq!(song_info.title, "Bubblegum K.K.");
    assert_eq!(song_info.wiki_url, "https://nookipedia.com/wiki/Bubblegum_K.K.");
    assert_eq!(song_info.number, 88);
    assert_eq!(song_info.image_url, "https://dodo.ac/np/images/6/69/Bubblegum_K.K._NH_Texture.png");
    assert_eq!(song_info.song_file_urls.len(), 7);
    assert_eq!(song_info.song_file_urls.get(&SongType::Live).unwrap(), "https://dodo.ac/np/images/6/6d/NH_Bubblegum_K.K._%28Live%29.flac");
    assert_eq!(song_info.song_file_urls.get(&SongType::Aircheck).unwrap(), "https://dodo.ac/np/images/3/30/NH_Bubblegum_K.K._%28Aircheck%2C_Hi-Fi%29.flac");
    assert_eq!(song_info.song_file_urls.get(&SongType::AircheckCheap).unwrap(), "https://dodo.ac/np/images/1/1f/NH_Bubblegum_K.K._%28Aircheck%2C_Cheap%29.flac");
    assert_eq!(song_info.song_file_urls.get(&SongType::AircheckRetro).unwrap(), "https://dodo.ac/np/images/a/ab/NH_Bubblegum_K.K._%28Aircheck%2C_Retro%29.flac");
    assert_eq!(song_info.song_file_urls.get(&SongType::AircheckPhono).unwrap(), "https://dodo.ac/np/images/e/e9/NH_Bubblegum_K.K._%28Aircheck%2C_Phono%29.flac");
    assert_eq!(song_info.song_file_urls.get(&SongType::MusicBox).unwrap(), "https://dodo.ac/np/images/d/d7/NL_Bubblegum_K.K._%28Music_Box%29.flac");
    assert_eq!(song_info.song_file_urls.get(&SongType::DjKkRemix).unwrap(), "https://dodo.ac/np/images/c/c1/HHP_Bubblegum_K.K._%28DJ_KK_Remix%29.flac");
}
