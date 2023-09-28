use iced::{
    widget::{column, container, scrollable, Button, TextInput},
    Element, Length,
};

use super::{App, Message};

#[derive(Debug, Default)]
pub struct StateLinkInput {
    pub youtube_link: String,
    pub discogs_link: String,
}

impl App {
    pub fn view_link_input<'a>(state: &'_ StateLinkInput) -> Element<'a, Message> {
        let yt_link_input = TextInput::new(
            "https://youtube.com/playlist?list=0123456789abcdef",
            state.youtube_link.as_str(),
        )
        .on_input(Message::YoutubeLinkInputChanged);

        let discogs_link_input = TextInput::new(
            "https://discogs.com/release/12345678-Artist-Name-Album-Name",
            state.discogs_link.as_str(),
        )
        .on_input(Message::DiscogsLinkInputChanged);

        let submit_button = Button::new("Scrape").on_press(Message::SubmitLinks {
            youtube: state.youtube_link.clone(),
            discogs: state.discogs_link.clone(),
        });

        let content = column![yt_link_input, discogs_link_input, submit_button]
            .spacing(20)
            .max_width(800);

        scrollable(
            container(content)
                .width(Length::Fill)
                .padding(40)
                .center_x(),
        )
        .into()
    }
}
