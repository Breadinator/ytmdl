# ytmdl
![Crates.io](https://img.shields.io/crates/v/ytmdl)
![docs.rs](https://img.shields.io/docsrs/ytmdl)
![GitHub](https://img.shields.io/github/license/Breadinator/ytmdl)

## Usage
Just run the executable and it should bring up the GUI. I recommend running it from a console for logging.

The first screen prompts for a YouTube playlist link and a Discogs release link.

Hitting the "Scrape" button will scrape the data then bring you to the screen where you can modify information.
Discogs escapes some characters (e.g. `&` becomes `&amp;`) so you might want to change that,
and if there are multiple artists with the same name it'll show up as something like "Artist (3)".

Hitting the "Download" button here will start the downloads.
It won't respond while doing this, but will continue to log to the console (hence why I recommend running it from the console).
This step took ~20s for a 6 track album for me, but sometimes it can take longer (I believe sometimes YouTube can be throttled if it detects suspicious behaviour).

## Environment variables
- `RUST_LOG`: see [env_logger](https://github.com/rust-cli/env_logger/) (if unset I've made it default to `ytmdl`, which will just print all logs from this module)
- `YTMDL_OUT_DIR`: directory that the final mp3s will be (defaults to your [downloads_directory](https://docs.rs/dirs/latest/dirs/fn.download_dir.html)`/ytmdl/`)
- `YTMDL_OVERWRITE`: whether should overwrite or not (defaults to `true`)

## Requirements
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) ([as an executable](https://github.com/yt-dlp/yt-dlp/releases))
- [ffmpeg](https://ffmpeg.org/)

## Installation
### Releases
See [GitHub releases](https://github.com/Breadinator/ytmdl/releases)

### Build from source
```
cargo install ytmdl
```
or
```
cargo install --git https://github.com/Breadinator/ytmdl
```

## Todo
- [x] ~~Automatically get the specific Discogs release page from the master page if provided~~
- [ ] If it fails to scrape discogs, just procede with empty data
- [ ] Scrape Discogs for data on specific songs (e.g. composers)
- [ ] Scrape the full album date (not in the JSON block currently parsed but is present on the site)
