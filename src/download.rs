#![allow(clippy::module_name_repetitions)]

use crate::{
    gui::view_modifying_data::StateModifyingData,
    scraping::{scrape_playlist, scrape_youtube},
    utils::sanitize_file_name,
};
use id3::{
    frame::{Picture, PictureType},
    Tag, TagLike,
};
use std::{
    borrow::Cow,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};
use tempdir::TempDir;
use thiserror::Error;
use url::Url;

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

fn get_ids(url: &str) -> Result<Vec<String>, DownloadError> {
    fn music_to_www(url: &str) -> Cow<str> {
        if let Ok(mut parsed_url) = Url::parse(url) {
            if parsed_url.host_str() == Some("music") && parsed_url.set_host(Some("www")).is_ok() {
                Cow::Owned(parsed_url.to_string())
            } else {
                Cow::Borrowed(url)
            }
        } else {
            Cow::Borrowed(url)
        }
    }
    let url = music_to_www(url);

    log::debug!("scraping album data from YouTube...");
    match scrape_playlist(&url) {
        Ok(scraped_playlist) => {
            let mut out = Vec::with_capacity(scraped_playlist.len());
            let mut ok = true;
            for track in scraped_playlist {
                if let Some(id) = track.id {
                    out.push(id);
                } else {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Ok(out);
            }
        }
        Err(err) => log::warn!("{err}"),
    }

    log::warn!("couldn't manually scrape the playlist, falling back to yt-dlp");
    Ok(scrape_youtube(&url)?.into_iter().map(|t| t.id).collect())
}

#[allow(
    clippy::missing_errors_doc,
    clippy::never_loop,
    clippy::too_many_lines,
    clippy::cast_possible_truncation
)]
pub fn download_album(state: &StateModifyingData) -> Result<(), DownloadError> {
    // this could be async
    // could also not be a gigafunc

    let started = Instant::now();

    let ids = get_ids(state.youtube_url.as_str())?;
    // let scraped_youtube = scrape_youtube(state.youtube_url.as_str())?;

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

    let tracks = ids.len();
    for (i, id) in ids.into_iter().enumerate() {
        // download from youtube
        log::info!(r#"Downloading {}/{}, id "{}"..."#, i + 1, tracks, id);
        let output = Command::new("yt-dlp")
            .args([
                "--audio-quality",
                "0",
                "--get-filename",
                "-P",
                tmp_dir.path().to_str().ok_or(DownloadError::TmpDirError)?,
                "-o",
                format!("{i}.%(ext)s").as_str(),
                format!("https://youtu.be/{id}").as_str(),
            ])
            .output()?;
        if !output.status.success() {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(DownloadError::YtdlpError(id));
        }
        let path = String::from_utf8_lossy(&output.stdout);
        let path = path.trim_end();

        log::debug!("Downloading {} to {}", id, path);
        let output = Command::new("yt-dlp")
            .args([
                "--audio-quality",
                "0",
                "-P",
                tmp_dir.path().to_str().ok_or(DownloadError::TmpDirError)?,
                "-o",
                format!("{i}.%(ext)s").as_str(),
                format!("https://youtu.be/{id}").as_str(),
            ])
            .output()?;
        if !output.status.success() {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(DownloadError::YtdlpError(id));
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
                return Err(DownloadError::FfmpegError(id));
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
