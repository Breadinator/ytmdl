use std::{borrow::Cow, str::FromStr};

use crate::utils::{
    download,
    selectors::{RELEASE_SCHEMA, SPAN, TD, TIME, TRACKLIST, VERSIONS_TABLE_LINK},
};
use id3::Timestamp;
use scraper::{html::Select, Html};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsTrack {
    pub number: i32,
    pub title: String,
    /// In the format "mm::ss", e.g. "2:44"
    pub duration: String,
}

#[derive(Debug, Clone)]
pub struct DiscogsAlbum {
    pub album_data: DiscogsAlbumData,
    pub tracks: Vec<Option<DiscogsTrack>>,
    pub released: Option<Timestamp>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsAlbumData {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@id")]
    pub id: String,
    pub name: String,
    #[serde(rename = "musicReleaseFormat")]
    pub music_release_format: String,
    pub genre: Vec<String>,
    pub description: Option<String>,
    #[serde(rename = "datePublished")]
    pub date_published: i32,
    #[serde(rename = "catalogNumber")]
    pub catalog_number: String,
    #[serde(rename = "recordLabel")]
    pub record_label: Vec<DiscogsNamedObject>,
    #[serde(rename = "releaseOf")]
    pub release_of: DiscogsReleaseOf,
    #[serde(rename = "releasedEvent")]
    pub released_event: DiscogsReleasedEvent,
    pub image: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsNamedObject {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsReleaseOf {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@id")]
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "datePublished")]
    pub date_published: i32,
    #[serde(rename = "byArtist")]
    pub by_artist: Vec<DiscogsNamedObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsReleasedEvent {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "startDate")]
    pub start_date: i32,
    pub location: DiscogsLocation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsLocation {
    #[serde(rename = "@type")]
    pub r#type: String,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum DiscogsScrapeError {
    #[error("{0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("couldn't find release schema script")]
    CouldntFindReleaseSchema,
    #[error("{0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("couldn't find release page from master page")]
    CouldntFindReleasePage,
}

/// Scrapes a Discogs master page to find a release
fn release_from_master(url: &str) -> Result<Cow<str>, DiscogsScrapeError> {
    fn first_release_in_select(selection: Select<'_, '_>) -> Option<String> {
        for s in selection {
            if let Some(link) = s.value().attr("href") {
                if link.starts_with("/release/") {
                    return Some(format!("https://www.discogs.com{link}"));
                }
            }
        }
        None
    }

    if url.contains("discogs.com/master") {
        let resp = download(url)?;
        let document = Html::parse_document(resp.text()?.as_str());

        let links = document.select(&VERSIONS_TABLE_LINK);
        first_release_in_select(links)
            .map(Cow::Owned)
            .ok_or(DiscogsScrapeError::CouldntFindReleasePage)
    } else {
        Ok(Cow::Borrowed(url))
    }
}

/// Scrapes Discogs for various album data
///
/// # Errors
/// - If it can't download the page at the given URL
/// - If there was no JSON script tag with the id `release_schema`
/// - If the JSON couldn't be parsed
pub fn scrape_discogs(url: &str) -> Result<DiscogsAlbum, DiscogsScrapeError> {
    let url = release_from_master(url)?;
    let resp = download(&url)?;
    let document = Html::parse_document(resp.text()?.as_str());

    let album_data = parse_release_schema(&document)?;
    let tracks = parse_tracks(&document);
    let released = parse_released(&document);

    Ok(DiscogsAlbum {
        album_data,
        tracks,
        released,
    })
}

fn parse_release_schema(document: &Html) -> Result<DiscogsAlbumData, DiscogsScrapeError> {
    serde_json::de::from_str(
        document
            .select(&RELEASE_SCHEMA)
            .next()
            .ok_or(DiscogsScrapeError::CouldntFindReleaseSchema)?
            .inner_html()
            .as_str(),
    )
    .map_err(Into::into)
}

fn parse_tracks(document: &Html) -> Vec<Option<DiscogsTrack>> {
    document
        .select(&TRACKLIST)
        .map(|track| {
            let tds: Vec<_> = track.select(&TD).collect();
            if tds.len() >= 4 {
                Some(DiscogsTrack {
                    number: tds[0].inner_html().parse().ok()?,
                    title: tds[2].select(&SPAN).next()?.inner_html(),
                    duration: tds[3].select(&SPAN).next()?.inner_html(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn parse_released(document: &Html) -> Option<Timestamp> {
    document
        .select(&TIME)
        .next()?
        .value()
        .attr("datetime")
        .map(FromStr::from_str)
        .and_then(Result::ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_basic() {
        let album =
            scrape_discogs("https://www.discogs.com/release/27651927-Odd-Eye-Circle-Version-Up")
                .unwrap();

        // test album data
        assert_eq!(album.album_data.name.as_str(), "Version Up");
        assert_eq!(
            &album.album_data.genre,
            &["Electronic".to_string(), "Pop".to_string()]
        );
        assert_eq!(
            &album.album_data.description.unwrap(),
            "Album title stylized as &amp;quot;ODD EYE CIRCLE &amp;lt;Version Up&amp;gt;.&amp;quot;" // idk what this escaping is lol
        );
        assert_eq!(album.album_data.date_published, 2023);
        assert_eq!(&album.album_data.record_label[0].name, "Modhaus");
        assert_eq!(
            &album.album_data.release_of.by_artist[0].name,
            "ODD EYE CIRCLE"
        );
        assert!(album.album_data.image.starts_with("https://i.discogs.com/"));

        // test tracks
        assert_eq!(album.tracks.len(), 6);
        let expected_titles = [
            "Did You Wait?",
            "Air Force One",
            "Je Ne Sais Quoi",
            "Lucid",
            "Love Me Like",
            "My Secret Playlist",
        ];
        let expected_durations = ["1:10", "2:44", "2:54", "3:34", "2:59", "2:33"];
        for (i, track) in album.tracks.iter().map(Option::as_ref).enumerate() {
            assert_eq!(track.map(|t| t.number), Some(i32::try_from(i).unwrap() + 1));
            assert_eq!(track.map(|t| t.title.as_str()), Some(expected_titles[i]));
            assert_eq!(
                track.map(|t| t.duration.as_str()),
                Some(expected_durations[i])
            );
        }
    }

    #[test]
    fn master_basic() {
        let master = r#"https://www.discogs.com/master/3166419-Odd-Eye-Circle-Version-Up"#;
        let release = release_from_master(master).unwrap();
        assert_eq!(
            &release,
            r#"https://www.discogs.com/release/27651927-Odd-Eye-Circle-Version-Up"#
        );

        let master = r#"https://www.discogs.com/release/27651927-Odd-Eye-Circle-Version-Up"#;
        let release = release_from_master(master).unwrap();
        assert_eq!(&release, master);
    }
}
