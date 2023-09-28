use super::{App, Message};
use crate::scraping::{DiscogsAlbum, DiscogsAlbumData};
use iced::{
    widget::{column, container, scrollable},
    Element, Length,
};

#[derive(Debug, Default)]
pub struct StateModifyingData {
    youtube_url: String,
    album_data: AlbumData,
    track_data: Vec<TrackData>,
}

#[derive(Debug, Default)]
struct AlbumData {
    pub name: String,
    pub artists: Vec<String>,
    pub labels: Vec<String>,
    pub genre: Vec<String>,
    pub style: Vec<String>,
    pub year: i32,
}

impl From<&DiscogsAlbumData> for AlbumData {
    fn from(discogs_album_data: &DiscogsAlbumData) -> Self {
        AlbumData {
            name: discogs_album_data.name.clone(),
            artists: discogs_album_data
                .release_of
                .by_artist
                .iter()
                .map(|a| a.name.clone())
                .collect(),
            labels: discogs_album_data
                .record_label
                .iter()
                .map(|a| a.name.clone())
                .collect(),
            genre: discogs_album_data.genre.clone(),
            style: Vec::new(), // todo
            year: discogs_album_data.date_published,
        }
    }
}

#[derive(Debug, Default)]
struct TrackData {
    pub name: String,
}

impl StateModifyingData {
    #[must_use]
    pub fn new(youtube_url: String, scraped_discogs: &DiscogsAlbum) -> Self {
        let album_data = AlbumData::from(&scraped_discogs.album_data);
        let track_data = Vec::with_capacity(scraped_discogs.tracks.len());

        Self {
            youtube_url,
            album_data,
            track_data,
        }
    }
}

impl App {
    pub fn view_modifying_data<'a>(state: &'_ StateModifyingData) -> Element<'a, Message> {
        let content = column![].spacing(20).max_width(800);
        scrollable(container(content).width(Length::Fill).padding(40)).into()
    }
}
