use super::{
    message::Message, view_link_input::StateLinkInput, view_modifying_data::StateModifyingData,
};
use crate::scraping::scrape_discogs;
use iced::{Application, Command, Element, Theme};

#[derive(Debug)]
pub enum App {
    /// Screen to give the link to the YouTube playlist and the Discogs page
    LinkInput(StateLinkInput),
    /// Page that lets a user modify the scraped data to fix errors
    ModifyingData(StateModifyingData),
    /// Loading screen while downloading the various files and adding the metadata
    Downloading,
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
                }
            }
            Message::DiscogsLinkInputChanged(new_text) => {
                if let Self::LinkInput(state) = self {
                    state.discogs_link = new_text;
                }
            }
            Message::SubmitLinks { youtube, discogs } => {
                if let Ok(scraped_discogs) = scrape_discogs(discogs.as_str()) {
                    *self = Self::ModifyingData(StateModifyingData::new(youtube, &scraped_discogs));
                }
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        match self {
            Self::LinkInput(state) => Self::view_link_input(state),
            Self::ModifyingData(state) => Self::view_modifying_data(state),
            Self::Downloading => Self::view_downloading(),
        }
    }
}

impl App {
    fn view_downloading<'a>() -> Element<'a, Message> {
        todo!()
    }
}
