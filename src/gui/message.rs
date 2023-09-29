#[derive(Debug, Clone)]
pub enum Message {
    // link submit view
    YoutubeLinkInputChanged(String),
    DiscogsLinkInputChanged(String),
    SubmitLinks { youtube: String, discogs: String },

    // modify data view
    ModifyDataInputChanged(ModifyDataInputChange),
    Download,
}

#[derive(Debug, Clone)]
pub enum ModifyDataInputChange {
    AlbumName(String),
    Artist(String),
    Label(String),
    Genre(String),
    Style(String),
    Year(String),
    Tracks { index: usize, value: String },
}
