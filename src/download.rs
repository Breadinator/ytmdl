use crate::{
    gui::view_modifying_data::StateModifyingData,
    scraping::{scrape_playlist, scrape_youtube},
    utils::{music_to_www, sanitize_file_name, SendableRawPointer},
};
use bytes::Bytes;
use id3::{
    frame::{Picture, PictureType},
    Tag, TagLike,
};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use reqwest::header::{HeaderValue, CONTENT_TYPE};
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
    #[error("{0:?}")]
    MultipleErrors(Vec<Self>),
}

/// Actually downloads all the tracks, converts them to mp3 and applies ID3 tags
///
/// # Errors
/// - If it can't determine the temp dir or output dir, or if either are invalid
/// - If [`get_ids`] fails
/// - If it can't generate the output file name of a track (using the yt-dlp CLI tool)
/// - If the yt-dlp CLI tool fails to download a track
/// - If ffmpeg fails to convert the file to an mp3
/// - If the ID3 tags fail being written to the file
/// - If the file can't be moved from the temp directory to the actual output
pub fn download_album(state: &StateModifyingData) -> Result<(), DownloadError> {
    let started = Instant::now();

    let (tmp_dir, out_dir) = where_dirs()?;
    let tmp_dir =
        SendableRawPointer::new(tmp_dir.path().to_str().ok_or(DownloadError::TmpDirError)?);
    let out_dir = SendableRawPointer::new(out_dir.as_path());
    let ids = get_ids(state.youtube_url.as_str())?;
    let num_tracks = ids.len();
    let (img, content_type) = get_image(state);
    let img = img.as_deref().map(SendableRawPointer::new);
    let content_type = content_type.as_deref().map(SendableRawPointer::new);
    let state = state.into();

    let errors: Vec<DownloadError> = crate::POOL.install(|| {
        ids.into_iter()
            .enumerate()
            .collect::<Vec<_>>()
            .into_par_iter()
            .filter_map(|(i, id)| {
                // SAFETY: none of the raw pointers sent here will be invalidated because all the
                // tasks are joined before the memory is deallocated
                unsafe {
                    handle_track(
                        state,
                        i,
                        num_tracks,
                        id,
                        tmp_dir,
                        out_dir,
                        img,
                        content_type,
                    )
                }
                .err()
            })
            .collect()
    });

    log::info!("Finished in {}s", started.elapsed().as_secs());

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DownloadError::MultipleErrors(errors))
    }
}

/// This downloads the file, sets its id3 tags, moves it to correct dir
///
/// # Safety
/// The arguments passed as [`SendableRawPointer`]s must be valid for the duration of the function.
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
unsafe fn handle_track(
    state: SendableRawPointer<StateModifyingData>,
    i: usize,
    num_tracks: usize,
    id: String,
    tmp_dir: SendableRawPointer<str>,
    out_dir: SendableRawPointer<Path>,
    img: Option<SendableRawPointer<[u8]>>,
    content_type: Option<SendableRawPointer<str>>,
) -> Result<(), DownloadError> {
    // SAFETY: these .get calls aren't guaranteed to be safe
    let state = state.get();
    let tmp_dir = tmp_dir.get();
    let out_dir = out_dir.get();
    let img = img.as_ref().map(|i| i.get());
    let content_type = content_type.as_ref().map(|ct| ct.get());
    // SAFETY: everything after here should be safe (assuming the above are valid)

    // download from youtube
    let path = generate_path_name(i, num_tracks, &id, tmp_dir)?;
    dl_from_yt(i, &id, &path, tmp_dir)?;

    // convert from webm or whatever to mp3
    let tmp_file_path = convert_to_mp3(&path, &id)?;

    // set id3 tags
    let tag = generate_tags(state, i, img, content_type);
    tag.write_to_path(&tmp_file_path, id3::Version::Id3v24)?;

    // copy to out dir
    move_to_out_dir(i, state, &tmp_file_path, out_dir)
}

fn get_ids(url: &str) -> Result<Vec<String>, DownloadError> {
    let url = music_to_www(url);

    log::debug!("scraping album data from YouTube...");
    match scrape_playlist(&url) {
        Ok(scraped_playlist) => {
            let mut out = Vec::with_capacity(scraped_playlist.len());
            let mut ok = true;
            for track in scraped_playlist.tracks {
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
                .get(CONTENT_TYPE)
                .map(HeaderValue::to_str)
                .and_then(Result::ok)
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
    if let Some(dr) = state.album_data.released {
        tag.set_date_released(dr);
    }
    tag.set_track((i + 1) as u32);
    tag.set_total_tracks(state.track_data.len() as u32);
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
