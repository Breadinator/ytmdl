#![allow(clippy::module_name_repetitions)]

use crate::{
    gui::view_modifying_data::StateModifyingData,
    scraping::{scrape_playlist, scrape_youtube},
    utils::sanitize_file_name,
};
use bytes::Bytes;
use id3::{
    frame::{Picture, PictureType},
    Tag, TagLike,
};
use std::{
    borrow::Cow,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    thread::{self, JoinHandle},
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
    #[error("{0:?}")]
    MultipleErrors(Vec<Self>),
}

#[allow(clippy::missing_errors_doc)]
pub fn download_album(state: &StateModifyingData) -> Result<(), DownloadError> {
    let started = Instant::now();

    let (tmp_dir, out_dir) = where_dirs()?;
    let ids = get_ids(state.youtube_url.as_str())?;
    let num_tracks = ids.len();
    let (img, content_type) = get_image(state);
    let state = Arc::new(state.clone()); // maybe Box::leak it

    let mut handles: Vec<JoinHandle<Result<(), DownloadError>>> = Vec::with_capacity(num_tracks);
    for (i, id) in ids.into_iter().enumerate() {
        let state = state.clone();
        let tmp_dir = tmp_dir.path().to_path_buf();
        let out_dir = out_dir.clone();
        let img = img.clone();
        let content_type = content_type.clone();

        handles.push(thread::spawn(move || {
            handle_track(
                state,
                i,
                num_tracks,
                &id,
                tmp_dir,
                out_dir,
                img,
                content_type,
            )
        }));
    }

    let mut errors = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(Err(err)) = handle.join() {
            errors.push(err);
        }
    }

    log::info!("Finished in {}s", started.elapsed().as_secs());

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DownloadError::MultipleErrors(errors))
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn handle_track(
    state: Arc<StateModifyingData>,
    i: usize,
    num_tracks: usize,
    id: &str,
    tmp_dir: PathBuf,
    out_dir: PathBuf,
    img: Option<Bytes>,
    content_type: Option<String>,
) -> Result<(), DownloadError> {
    // download from youtube
    let path = generate_path_name(i, num_tracks, id, &tmp_dir.to_string_lossy())?;
    dl_from_yt(i, id, &path, &tmp_dir.to_string_lossy())?;

    // convert from webm or whatever to mp3
    let tmp_file_path = convert_to_mp3(&path, id)?;

    // set id3 tags
    let tag = generate_tags(&state, i, img.as_deref(), content_type.as_deref());
    tag.write_to_path(tmp_file_path.as_path(), id3::Version::Id3v24)?;

    // copy to out dir
    move_to_out_dir(i, &state, &tmp_file_path, &out_dir)
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

fn get_image(state: &StateModifyingData) -> (Option<Bytes>, Option<String>) {
    let mut img = None;
    let mut content_type = None;

    match reqwest::blocking::get(&state.album_data.image) {
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

    (img, content_type)
}

fn where_dirs() -> Result<(TempDir, PathBuf), DownloadError> {
    // IMPORTANT: `TempDir` deleted dir on `drop`;
    // moving in return so is fine but don't change to be PathBuf or String
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
    Ok((tmp_dir, out_dir))
}

fn generate_path_name(
    i: usize,
    num_tracks: usize,
    id: &str,
    tmp_dir: &str,
) -> Result<String, DownloadError> {
    // download from youtube
    log::info!(r#"Downloading {}/{}, id "{}"..."#, i + 1, num_tracks, id);
    let output = Command::new("yt-dlp")
        .args([
            "--audio-quality",
            "0",
            "--get-filename",
            "-P",
            tmp_dir,
            "-o",
            format!("{i}.%(ext)s").as_str(),
            format!("https://youtu.be/{id}").as_str(),
        ])
        .output()?;
    if !output.status.success() {
        log::error!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(DownloadError::YtdlpError(id.to_string()));
    }
    let path = String::from_utf8_lossy(&output.stdout);
    let path = path.trim_end();
    Ok(path.to_string())
}

fn dl_from_yt(i: usize, id: &str, path: &str, tmp_dir: &str) -> Result<(), DownloadError> {
    log::debug!("Downloading {} to {}", id, path);
    let output = Command::new("yt-dlp")
        .args([
            "--audio-quality",
            "0",
            "-P",
            tmp_dir,
            "-o",
            format!("{i}.%(ext)s").as_str(),
            format!("https://youtu.be/{id}").as_str(),
        ])
        .output()?;
    if !output.status.success() {
        log::error!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(DownloadError::YtdlpError(id.to_string()));
    }

    Ok(())
}

fn convert_to_mp3(old_path: &str, id: &str) -> Result<PathBuf, DownloadError> {
    let mut path = PathBuf::from(old_path);
    if Path::new(old_path)
        .extension()
        .map_or(false, |ext| ext.eq_ignore_ascii_case("mp3"))
    {
        Ok(old_path.into())
    } else {
        path.set_extension("mp3");
        log::debug!(
            r#"Converting "{}" to "{}""#,
            old_path,
            path.to_string_lossy()
        );
        let output = Command::new("ffmpeg")
            .args(["-i", old_path, path.to_string_lossy().as_ref()])
            .output()?;
        if output.status.success() {
            Ok(path)
        } else {
            log::error!("{}", String::from_utf8_lossy(&output.stderr));
            Err(DownloadError::FfmpegError(id.to_string()))
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn generate_tags(
    state: &StateModifyingData,
    i: usize,
    img: Option<&[u8]>,
    content_type: Option<&str>,
) -> Tag {
    let mut tag = Tag::new();
    tag.set_album(&state.album_data.name);
    tag.set_year(state.album_data.year);
    tag.set_track((i + 1) as u32);
    tag.set_artist(&state.album_data.artist);
    tag.set_genre(&state.album_data.genre);
    tag.set_title(&state.track_data[i].name);
    if let (Some(content_type), Some(img)) = (content_type, img) {
        tag.add_frame(Picture {
            mime_type: content_type.to_string(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: img.to_vec(),
        });
    }
    tag.set_album_artist(&state.album_data.artist);
    tag
}

fn move_to_out_dir(
    i: usize,
    state: &StateModifyingData,
    old_path: &Path,
    out_dir: &Path,
) -> Result<(), DownloadError> {
    let mut out_file_path = out_dir.to_path_buf();
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
        old_path.to_string_lossy(),
        out_file_path.to_string_lossy()
    );
    if !old_path.exists() {
        log::warn!(r#""{}" doesn't exist"#, old_path.to_string_lossy());
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
            fs::remove_file(old_path)?;
            return Ok(());
        }
    }
    fs::copy(old_path, out_file_path)?;
    log::debug!(r#"Deleting temp file"#);
    fs::remove_file(old_path)?;

    Ok(())
}
