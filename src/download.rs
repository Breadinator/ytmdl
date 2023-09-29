#![allow(clippy::module_name_repetitions)]

use crate::{gui::view_modifying_data::StateModifyingData, scraping::scrape_youtube};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tempdir::TempDir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("{0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("{0}")]
    ScrapeYoutubeError(#[from] crate::scraping::ScrapeYoutubeError),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("ytdlp error when downloading {0}")]
    YtdlpError(String),
    #[error("ffmpeg error converting {0}")]
    FfmpegError(String),
    #[error("some error with the temp dir")]
    TmpDirError,
}

#[allow(
    clippy::missing_errors_doc,
    clippy::never_loop,
    clippy::missing_panics_doc
)]
pub fn download(state: &StateModifyingData) -> Result<(), DownloadError> {
    // this could be async

    log::info!("Scraping album data from YouTube...");
    let scraped_youtube = scrape_youtube(state.youtube_url.as_str())?;

    let tmp_dir = TempDir::new("ytmdl")?;

    for (i, video) in scraped_youtube.into_iter().enumerate() {
        log::info!("Downloading {}...", video.id);
        let output = Command::new("yt-dlp")
            .args([
                "--audio-quality",
                "0",
                "--get-filename",
                "-P",
                tmp_dir.path().to_str().ok_or(DownloadError::TmpDirError)?,
                "-o",
                format!("{i}.%(ext)s").as_str(),
                video.id.as_str(),
            ])
            .output()?;
        if !output.status.success() {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(DownloadError::YtdlpError(video.id));
        }
        let path = String::from_utf8_lossy(&output.stdout);
        let path = path.trim_end();

        log::debug!("Downloading {} to {}", video.id, path);
        let output = Command::new("yt-dlp")
            .args([
                "--audio-quality",
                "0",
                "-P",
                tmp_dir.path().to_str().ok_or(DownloadError::TmpDirError)?,
                "-o",
                format!("{i}.%(ext)s").as_str(),
                video.id.as_str(),
            ])
            .output()?;
        if !output.status.success() {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(DownloadError::YtdlpError(video.id));
        }

        let mut new_path = PathBuf::from(path);
        new_path.set_extension("mp3");
        log::debug!("Converting {} to {}", path, new_path.to_string_lossy());
        let output = Command::new("ffmpeg")
            .args(["-i", path, new_path.to_string_lossy().as_ref()])
            .output()?;
        if !output.status.success() {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(DownloadError::FfmpegError(video.id));
        }

        assert!(new_path.exists());

        break;
    }
    Ok(())
}
