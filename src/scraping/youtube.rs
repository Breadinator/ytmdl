use crate::utils::reduce_vec_of_results;
use serde::Deserialize;
use serde_json::Value;
use std::{io, process::Command};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScrapeYoutubeError {
    #[error("{0}")]
    IoError(#[from] io::Error),
    #[error("{0}")]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
pub struct YoutubeVideo {
    pub id: String,
    pub title: String,
    pub full_title: Option<String>,
    pub thumbnail: String,
    pub thumbnails: Vec<YoutubeThumbnail>,
    pub description: Option<String>,
    pub channel_id: String,
    pub channel_url: String,
    pub duration: Option<i32>,
    pub duration_string: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub subtitles: Value, // seems to be a hashmap of some kind, probably lang or timestamp as keys
    pub album: String,
    pub artist: String,
    pub track: String,
    pub release_year: Option<i32>,
    pub release_date: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct YoutubeThumbnail {
    pub url: String,
    pub preference: i32,
    pub id: String,
}

/// Uses the yt-dlp CLI tool to scrape information about a Youtube video
///
/// # Errors
/// - If the yt-dlp command fails
/// - If it can't parse the returned JSON
pub fn scrape_youtube(url: &str) -> Result<Vec<YoutubeVideo>, ScrapeYoutubeError> {
    let output = Command::new("yt-dlp")
        .args(["--skip-download", "--dump-json", url])
        .output()?;

    let video_datas: Vec<Result<YoutubeVideo, _>> = output
        .stdout
        .split(|c| *c == b'\n')
        .filter(|s| !s.is_empty())
        .map(serde_json::de::from_slice)
        .collect();

    reduce_vec_of_results(video_datas).map_err(ScrapeYoutubeError::SerdeJsonError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let output = scrape_youtube(
            "https://www.youtube.com/playlist?list=OLAK5uy_mZcxjzRvOZAUa2H6Pf8LVvyLDGeBSdmJQ",
        )
        .unwrap();

        let expected_titles = [
            "Did You Wait? (기다렸어?)",
            "Air Force One (Air Force One)",
            "Je Ne Sais Quoi (Je Ne Sais Quoi)",
            "Lucid (Lucid)",
            "Love Me Like (Love Me Like)",
            "My Secret Playlist (My Secret Playlist)",
        ];

        let expected_durations = [70, 164, 174, 214, 179, 153];
        let expected_duration_strings = ["1:10", "2:44", "2:54", "3:34", "2:59", "2:33"];

        assert_eq!(output.len(), 6);
        for (i, track) in output.into_iter().enumerate() {
            assert_eq!(track.title, expected_titles[i]);
            assert_eq!(track.duration.unwrap(), expected_durations[i]);
            assert_eq!(
                track.duration_string.unwrap().as_str(),
                expected_duration_strings[i]
            );
            assert_eq!(track.release_year.unwrap(), 2023);
            assert_eq!(track.categories, vec![String::from("Music")]);
            assert!(track
                .description
                .unwrap()
                .starts_with("Provided to YouTube by Kakao Entertainment"));
            assert!(track.thumbnail.starts_with("https://i.ytimg.com/vi"));
        }
    }
}
