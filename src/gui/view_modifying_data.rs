use super::{App, Message, ModifyDataInputChange};
use crate::{
    scraping::{
        scrape_playlist, DiscogsAlbum, DiscogsAlbumData, PlaylistItem, ScrapeYoutubePlaylistError,
    },
    utils::music_to_www,
};
use iced::{
    widget::{column, container, scrollable, Button, Column, Rule, TextInput},
    Element, Length,
};

#[derive(Debug, Clone, Default)]
pub struct StateModifyingData {
    pub youtube_url: String,
    pub album_data: AlbumData,
    pub track_data: Vec<TrackData>,
}

#[derive(Debug, Clone)]
pub struct AlbumData {
    pub name: String,
    pub artist: String,
    pub genre: String,
    pub year: i32,
    pub image: String,
}

impl Default for AlbumData {
    fn default() -> Self {
        Self {
            name: String::new(),
            artist: String::new(),
            genre: String::new(),
            year: crate::utils::current_year(),
            image: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackData {
    pub name: String,
}

impl From<PlaylistItem> for TrackData {
    fn from(value: PlaylistItem) -> Self {
        Self {
            name: value.title.unwrap_or_default(),
        }
    }
}

impl From<&DiscogsAlbumData> for AlbumData {
    fn from(discogs_album_data: &DiscogsAlbumData) -> Self {
        AlbumData {
            name: discogs_album_data.name.clone(),
            artist: discogs_album_data.release_of.by_artist.iter().fold(
                String::new(),
                |acc, artist| {
                    if acc.is_empty() {
                        artist.name.clone()
                    } else {
                        acc + "; " + &artist.name
                    }
                },
            ),
            genre: discogs_album_data
                .genre
                .iter()
                .fold(String::new(), |acc, genre| {
                    if acc.is_empty() {
                        genre.clone()
                    } else {
                        acc + "; " + &genre
                    }
                }),
            year: discogs_album_data.date_published,
            image: discogs_album_data.image.clone(),
        }
    }
}

impl StateModifyingData {
    #[must_use]
    pub fn new(youtube_url: String, scraped_discogs: &DiscogsAlbum) -> Self {
        let album_data = AlbumData::from(&scraped_discogs.album_data);
        let mut track_data = Vec::with_capacity(scraped_discogs.tracks.len());
        for track in &scraped_discogs.tracks {
            if let Some(track) = track {
                track_data.push(TrackData {
                    name: track.title.clone(),
                });
            } else {
                log::error!("failed to parse track");
            }
        }

        Self {
            youtube_url,
            album_data,
            track_data,
        }
    }

    /// Fails if [`scrape_playlist`] fails (used to see how many tracks in the album)
    #[allow(clippy::missing_errors_doc)]
    pub fn new_without_discogs(youtube_url: String) -> Result<Self, ScrapeYoutubePlaylistError> {
        scrape_playlist(&music_to_www(&youtube_url)).map(|playlist_data| Self {
            youtube_url,
            album_data: AlbumData {
                name: playlist_data.title,
                artist: playlist_data.artist,
                ..AlbumData::default()
            },
            track_data: playlist_data.tracks.into_iter().map(Into::into).collect(),
        })
    }
}

impl App {
    #[must_use]
    pub fn view_modifying_data<'a>(state: &'_ StateModifyingData) -> Element<'a, Message> {
        // submit buttons
        let download_button: Button<'_, Message> =
            Button::new("Download").on_press(Message::Download);

        // album data
        let album_name_input: TextInput<'_, Message> =
            TextInput::new("Album name", state.album_data.name.as_str())
                .on_input(|s| Message::ModifyDataInputChanged(ModifyDataInputChange::AlbumName(s)));
        let album_artist_input = TextInput::new("Artists", &state.album_data.artist)
            .on_input(|s| Message::ModifyDataInputChanged(ModifyDataInputChange::Artist(s)));
        let album_date_input = TextInput::new("Date", &format!("{}", state.album_data.year))
            .on_input(|s| Message::ModifyDataInputChanged(ModifyDataInputChange::Year(s)));
        let album_genre_input = TextInput::new("Genre", &state.album_data.genre)
            .on_input(|s| Message::ModifyDataInputChanged(ModifyDataInputChange::Genre(s)));
        let album_cover_url_input = TextInput::new("Album Cover URL", &state.album_data.image)
            .on_input(|s| Message::ModifyDataInputChanged(ModifyDataInputChange::Image(s)));

        let mut content: Column<'_, Message> = column![
            download_button,
            Rule::horizontal(4),
            album_name_input,
            album_artist_input,
            album_date_input,
            album_genre_input,
            album_cover_url_input,
            Rule::horizontal(4)
        ]
        .spacing(20)
        .max_width(800);

        // tracks
        for (i, track) in state.track_data.iter().enumerate() {
            let track_change_input =
                TextInput::new(format!("Track {}", i + 1).as_str(), track.name.as_str()).on_input(
                    move |s| {
                        Message::ModifyDataInputChanged(ModifyDataInputChange::Tracks {
                            index: i,
                            value: s,
                        })
                    },
                );
            content = content.push(track_change_input);
        }

        scrollable(container(content).width(Length::Fill).padding(40)).into()
    }
}
