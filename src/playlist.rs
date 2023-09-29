use crate::parsing::{consume, consume_mutually_exclusive};

#[must_use]
pub fn validate(url: impl AsRef<str>) -> bool {
    match parse_id_from_url(url.as_ref()) {
        Some(_id) => todo!(),
        None => false,
    }
}

/// Parses out the playlist ID from a playlist
///
/// # Examples
/// ```
/// let url = r#"https://youtube.com/playlist?list=OLAK5uy_mZcxjzRvOZAUa2H6Pf8LVvyLDGeBSdmJQ&si=1d2ju9812hjdo"#;
/// let id = ytmdl::playlist::parse_id_from_url(url);
/// assert!(id.is_some());
/// assert_eq!(id.unwrap(), r#"OLAK5uy_mZcxjzRvOZAUa2H6Pf8LVvyLDGeBSdmJQ"#);
/// ```
#[must_use]
pub fn parse_id_from_url(url: &str) -> Option<String> {
    // it would probably be better to just do this with indexes and whatnot, then return a slice

    let mut url = itertools::peek_nth(url.chars());

    // eat up the stuff that doesn't matter
    consume_mutually_exclusive(&mut url, &["https://", "http://"]);
    consume_mutually_exclusive(&mut url, &["www.", "music."]);
    consume(&mut url, "youtube.com/playlist?list=");

    let mut s = String::with_capacity(64);
    for ch in url {
        if ch == '&' {
            break;
        }
        s.push(ch);
    }

    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
