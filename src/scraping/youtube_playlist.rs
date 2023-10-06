use crate::utils::{download, selectors::SCRIPT};
use scraper::Html;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScrapeYoutubePlaylistError {
    #[error("{0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("selector parse error")]
    DeserializeError(#[from] serde_json::Error),
    #[error("missing valid `ytInitialData` script")]
    MissingScript,
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistItem {
    pub title: Option<String>,
    pub id: Option<String>,
}

fn extract_playlist_data(json: &Value) -> Option<&Value> {
    json.get("contents")?
        .get("twoColumnBrowseResultsRenderer")?
        .get("tabs")?
        .get(0)?
        .get("tabRenderer")?
        .get("content")?
        .get("sectionListRenderer")?
        .get("contents")?
        .get(0)?
        .get("itemSectionRenderer")?
        .get("contents")?
        .get(0)?
        .get("playlistVideoListRenderer")?
        .get("contents")
}

fn extract_playlist_item(extracted_json: &Value) -> PlaylistItem {
    fn extract_title(j: &Value) -> Option<String> {
        if let Value::String(title) = j.get("title")?.get("runs")?.get(0)?.get("text")? {
            Some(title.clone())
        } else {
            None
        }
    }
    fn extract_id(j: &Value) -> Option<String> {
        if let Some(Value::String(id)) = j.get("videoId") {
            Some(id.clone())
        } else {
            None
        }
    }

    if let Some(extracted_json) = extracted_json.get("playlistVideoRenderer") {
        PlaylistItem {
            title: extract_title(extracted_json),
            id: extract_id(extracted_json),
        }
    } else {
        PlaylistItem::default()
    }
}

/// Attempts to scrape out playlist information from the given link.
///
/// # Errors
/// - If it can't actually download the request (via [reqwest])
/// - If it can't find a valid script tag (whose contents should be `var ytInitialData = <...>;` where `<...>` is valid JSON)
pub fn scrape_playlist(url: &str) -> Result<Vec<PlaylistItem>, ScrapeYoutubePlaylistError> {
    let resp = download(url)?.text()?;
    let doc = Html::parse_document(resp.as_str());

    for script in doc.select(&SCRIPT) {
        let inner = script.inner_html();

        if let Some(Ok(json)) = inner
            .strip_prefix("var ytInitialData = ")
            .and_then(|s| {
                if s.ends_with(';') {
                    s.strip_suffix(';')
                } else {
                    Some(s)
                }
            })
            .map(serde_json::from_str::<Value>)
        {
            if let Some(Value::Array(tracks)) = extract_playlist_data(&json) {
                return Ok(tracks.iter().map(extract_playlist_item).collect());
            }
        }
    }

    Err(ScrapeYoutubePlaylistError::MissingScript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_a() {
        let playlist = scrape_playlist(
            r#"https://www.youtube.com/playlist?list=OLAK5uy_mZcxjzRvOZAUa2H6Pf8LVvyLDGeBSdmJQ"#,
        )
        .unwrap();

        assert_eq!(playlist.len(), 6);
        for track in playlist {
            assert_ne!(track.title, None);
            assert_ne!(track.id, None);
        }
    }
}
