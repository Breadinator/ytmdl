#![allow(clippy::module_name_repetitions)]

use crate::{
    gui::view_modifying_data::StateModifyingData, scraping::scrape_youtube,
    utils::sanitize_file_name,
};
use id3::{
    frame::{Picture, PictureType},
    Tag, TagLike,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};
use tempdir::TempDir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
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
    #[error("{0}")]
    Id3Error(#[from] id3::Error),
}

#[allow(
    clippy::missing_errors_doc,
    clippy::never_loop,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation
)]
pub fn download(state: &StateModifyingData) -> Result<(), DownloadError> {
    // this could be async
    // could also not be a gigafunc

    let started = Instant::now();

    log::debug!("Scraping album data from YouTube...");
    let scraped_youtube = scrape_youtube(state.youtube_url.as_str())?;

    let img_req = reqwest::blocking::get(&state.album_data.image);
    let mut img = None;
    let mut content_type = None;
    match img_req {
        Ok(resp) => {
            content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|a| a.to_str().ok())
                .map(String::from);
            img = resp.bytes().ok();
        }
        Err(err) => log::error!("error when downloading album art: {err}"),
    }

    let tmp_dir = TempDir::new("ytmdl")?;
    let out_dir = env::var("YTMDL_OUT_DIR").map_or_else(
        |_| {
            let mut p = env::current_dir().unwrap_or_default();
            p.push("ytmdl");
            p
        },
        PathBuf::from,
    );
    fs::create_dir_all(out_dir.as_path())?;

    let tracks = scraped_youtube.len();
    for (i, video) in scraped_youtube.into_iter().enumerate() {
        // download from youtube
        log::info!(r#"Downloading {}/{}, id "{}"..."#, i + 1, tracks, video.id);
        let output = Command::new("yt-dlp")
            .args([
                "--audio-quality",
                "0",
                "--get-filename",
                "-P",
                tmp_dir.path().to_str().ok_or(DownloadError::TmpDirError)?,
                "-o",
                format!("{i}.%(ext)s").as_str(),
                format!("https://youtu.be/{}", video.id).as_str(),
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
                format!("https://youtu.be/{}", video.id).as_str(),
            ])
            .output()?;
        if !output.status.success() {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(DownloadError::YtdlpError(video.id));
        }

        // convert from webm or whatever to mp3
        let mut tmp_file_path = PathBuf::from(path);
        if !Path::new(path)
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("mp3"))
        {
            tmp_file_path.set_extension("mp3");
            log::debug!(
                r#"Converting "{}" to "{}""#,
                path,
                tmp_file_path.to_string_lossy()
            );
            let output = Command::new("ffmpeg")
                .args(["-i", path, tmp_file_path.to_string_lossy().as_ref()])
                .output()?;
            if !output.status.success() {
                log::error!("{}", String::from_utf8_lossy(&output.stderr));
                return Err(DownloadError::FfmpegError(video.id));
            }
        }

        // set id3 tags
        let mut tag = Tag::new();
        tag.set_album(&state.album_data.name);
        tag.set_year(state.album_data.year);
        tag.set_track((i + 1) as u32);
        tag.set_artist(&state.album_data.artist);
        tag.set_genre(&state.album_data.genre);
        tag.set_title(&state.track_data[i].name);
        if let (Some(content_type), Some(img)) = (content_type.as_ref(), img.as_ref()) {
            tag.add_frame(Picture {
                mime_type: content_type.clone(),
                picture_type: PictureType::CoverFront,
                description: String::new(),
                data: img.clone().into(),
            });
        }
        tag.set_album_artist(&state.album_data.artist);
        tag.write_to_path(tmp_file_path.as_path(), id3::Version::Id3v24)?;

        // copy to out dir
        let mut out_file_path = out_dir.clone();
        out_file_path.push(
            sanitize_file_name(
                format!(
                    "{} - {} - {}.mp3",
                    state.album_data.artist, state.album_data.name, state.track_data[i].name
                )
                .as_str(),
            )
            .as_ref(),
        );
        log::debug!(
            r#"Copying "{}" to "{}""#,
            tmp_file_path.to_string_lossy(),
            out_file_path.to_string_lossy()
        );
        if !tmp_file_path.exists() {
            log::warn!(r#""{}" doesn't exist"#, tmp_file_path.to_string_lossy());
        }
        if out_file_path.exists() {
            if env::var("YTMDL_OVERWRITE").map_or(true, |v| v.as_str() == "true") {
                log::debug!(r#"Removing existing "{}""#, out_file_path.to_string_lossy());
                fs::remove_file(out_file_path.as_path())?;
            } else {
                log::warn!(
                    r#""{}" already exists; skipping"#,
                    out_file_path.to_string_lossy()
                );
                fs::remove_file(tmp_file_path.as_path())?;
                continue;
            }
        }
        fs::copy(tmp_file_path.as_path(), out_file_path)?;
        log::debug!(r#"Deleting temp file"#);
        fs::remove_file(tmp_file_path)?;

        println!();
    }

    log::info!("Finished in {}s", started.elapsed().as_secs());

    Ok(())
}
