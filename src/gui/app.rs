use super::{
    message::Message, view_link_input::StateLinkInput, view_modifying_data::StateModifyingData,
    ModifyDataInputChange,
};
use crate::scraping::scrape_discogs;
use iced::{Application, Command, Element, Theme};

#[derive(Debug)]
pub enum App {
    /// Screen to give the link to the YouTube playlist and the Discogs page
    LinkInput(StateLinkInput),
    /// Page that lets a user modify the scraped data to fix errors
    ModifyingData(StateModifyingData),
}

impl Default for App {
    fn default() -> Self {
        Self::LinkInput(StateLinkInput::default())
    }
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (App::default(), Command::batch(vec![]))
    }

    fn title(&self) -> String {
        String::from("ytmdl")
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::YoutubeLinkInputChanged(new_text) => {
                if let Self::LinkInput(state) = self {
                    state.youtube_link = new_text;
                } else {
                    log::warn!(
                        "Received `Message::YoutubeLinkInputChanged` when not in LinkInput state"
                    );
                }
            }
            Message::DiscogsLinkInputChanged(new_text) => {
                if let Self::LinkInput(state) = self {
                    state.discogs_link = new_text;
                } else {
                    log::warn!(
                        "Received `Message::DiscogsLinkInputChanged` when not in LinkInput state"
                    );
                }
            }
            Message::SubmitLinks { youtube, discogs } => match scrape_discogs(discogs.as_str()) {
                Ok(scraped_discogs) => {
                    *self = Self::ModifyingData(StateModifyingData::new(youtube, &scraped_discogs));
                }
                Err(err) => log::error!("{err}"),
            },
            Message::ModifyDataInputChanged(change) => {
                if let App::ModifyingData(data) = self {
                    match change {
                        ModifyDataInputChange::AlbumName(s) => data.album_data.name = s,
                        ModifyDataInputChange::Artist(s) => data.album_data.artist = s,
                        ModifyDataInputChange::Label(s) => data.album_data.label = s,
                        ModifyDataInputChange::Genre(s) => data.album_data.genre = s,
                        ModifyDataInputChange::Year(s) => {
                            if let Ok(y) = s.parse() {
                                data.album_data.year = y;
                            }
                        }
                        ModifyDataInputChange::Tracks { index, value } => {
                            data.track_data[index].name = value;
                        }
                    }
                } else {
                    log::warn!(
                        "Received `Message::ModifyDataInputChanged` when not in ModifyingData state"
                    );
                }
            }
            Message::Download => {
                if let App::ModifyingData(state) = self {
                    if let Err(err) = crate::download_album(state) {
                        log::error!("{err}");
                    }
                    *self = Self::LinkInput(StateLinkInput::default());
                } else {
                    log::warn!("Received `Message::Download` when not in ModifyingData state");
                }
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        match self {
            Self::LinkInput(state) => Self::view_link_input(state),
            Self::ModifyingData(state) => Self::view_modifying_data(state),
        }
    }
}
