use std::{borrow::Cow, ffi::OsStr};

/// If all given results are `Ok`, returns `Ok(vec![ok_values])`,
/// else it returns the first error in the `Vec`
#[allow(clippy::missing_errors_doc)]
pub fn reduce_vec_of_results<T, E>(results: Vec<Result<T, E>>) -> Result<Vec<T>, E> {
    let mut out = Vec::with_capacity(results.len());

    for res in results {
        match res {
            Ok(val) => out.push(val),
            Err(err) => return Err(err),
        }
    }

    Ok(out)
}

static ILLEGAL_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

fn contains_illegal_chars(path: impl AsRef<OsStr>) -> bool {
    path.as_ref()
        .to_str()
        .unwrap_or_default()
        .contains(|c| ILLEGAL_CHARS.contains(&c))
}

/*#[must_use]
pub fn sanitize_path(path: &Path) -> Cow<Path> {
    if contains_illegal_chars(path) {
        let mut out = PathBuf::with_capacity(path.iter().count());

        for part in path {
            if contains_illegal_chars(part) {
                if let Some(part) = part.to_str() {
                    let mut s = String::with_capacity(part.len());
                    for ch in part.chars() {
                        if !ILLEGAL_CHARS.contains(&ch) {
                            s.push(ch);
                        }
                    }
                    out.push(s);
                }
            } else {
                out.push(part);
            }
        }

        Cow::Owned(out)
    } else {
        Cow::Borrowed(path)
    }
}*/

#[must_use]
pub fn sanitize_file_name(name: &str) -> Cow<str> {
    if contains_illegal_chars(name) {
        let mut out = String::with_capacity(name.len());
        for ch in name.chars() {
            if !ILLEGAL_CHARS.contains(&ch) {
                out.push(ch);
            }
        }
        Cow::Owned(out)
    } else {
        Cow::Borrowed(name)
    }
}
