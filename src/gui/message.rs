#[derive(Debug, Clone)]
pub enum Message {
    YoutubeLinkInputChanged(String),
    DiscogsLinkInputChanged(String),
    SubmitLinks { youtube: String, discogs: String },
}
