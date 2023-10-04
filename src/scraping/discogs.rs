use std::borrow::Cow;

use crate::utils::download;
use scraper::{html::Select, Html, Selector};
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
    pub id: String,
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
    #[error("invalid selector")]
    SelectorError,
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

        let selector_versions_table_link =
            Selector::parse(r#"section#versions table a.link_1ctor"#)
                .map_err(|_| DiscogsScrapeError::SelectorError)?;

        let links = document.select(&selector_versions_table_link);
        let link = first_release_in_select(links);

        link.map(Cow::Owned)
            .ok_or(DiscogsScrapeError::CouldntFindReleasePage)
    } else {
        Ok(Cow::Borrowed(url))
    }
}

/// Scrapes Discogs for various album data
///
/// # Errors
/// - If it can't download the page at the given URL
/// - If any of the various selectors fail to be compiled
/// - If there was no JSON script tag with the id `release_schema`
/// - If the JSON couldn't be parsed
pub fn scrape_discogs(url: &str) -> Result<DiscogsAlbum, DiscogsScrapeError> {
    let url = release_from_master(url)?;
    let resp = download(&url)?;
    let document = Html::parse_document(resp.text()?.as_str());

    // these could probably be lazy statics/once cells but not being called enough to matter
    let selector_release_schema = Selector::parse(r#"script#release_schema"#)
        .map_err(|_| DiscogsScrapeError::SelectorError)?;
    let selector_tracklist = Selector::parse(r#"section#release-tracklist tr"#)
        .map_err(|_| DiscogsScrapeError::SelectorError)?;
    let selector_tr = Selector::parse(r#"td"#).map_err(|_| DiscogsScrapeError::SelectorError)?;
    let selector_span =
        Selector::parse(r#"span"#).map_err(|_| DiscogsScrapeError::SelectorError)?;

    // extract album data
    let album_data: DiscogsAlbumData = serde_json::de::from_str(
        document
            .select(&selector_release_schema)
            .next()
            .ok_or(DiscogsScrapeError::CouldntFindReleaseSchema)?
            .inner_html()
            .as_str(),
    )?;

    // extract tracks
    let tracklist = document.select(&selector_tracklist);
    let tracks = tracklist
        .map(|track| {
            let tds: Vec<_> = track.select(&selector_tr).collect();
            if tds.len() >= 4 {
                let number = tds[0].inner_html().parse().ok()?;
                let title = tds[2].select(&selector_span).next()?.inner_html();
                let duration = tds[3].select(&selector_span).next()?.inner_html();

                Some(DiscogsTrack {
                    number,
                    title,
                    duration,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(DiscogsAlbum { album_data, tracks })
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
            album.album_data.genre.as_slice(),
            &[String::from("Electronic"), String::from("Pop")]
        );
        assert_eq!(
            album.album_data.description.unwrap().as_str(),
            "Album title stylized as &amp;quot;ODD EYE CIRCLE &amp;lt;Version Up&amp;gt;.&amp;quot;" // idk what this escaping is lol
        );
        assert_eq!(album.album_data.date_published, 2023);
        assert_eq!(album.album_data.record_label[0].name.as_str(), "Modhaus");
        assert_eq!(
            album.album_data.release_of.by_artist[0].name.as_str(),
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
