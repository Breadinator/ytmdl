use std::{borrow::Cow, ffi::OsStr};

use reqwest::blocking::{Client, Response};

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

/// Makes a get request via [reqwest] using a fake user agent
#[allow(clippy::missing_errors_doc)]
pub fn download(url: &str) -> Result<Response, reqwest::Error> {
    let client = Client::builder().user_agent("Chrome/116.0.0.0").build()?; // lol
    client.get(url).send()
}

/// Wrapper around a `*const T` that allows it to be sent across threads.
pub struct SendableRawPointer<T: ?Sized>(*const T);
unsafe impl<T: ?Sized> Send for SendableRawPointer<T> {}
impl<T: ?Sized> Copy for SendableRawPointer<T> {}

impl<T: ?Sized> SendableRawPointer<T> {
    #[must_use]
    pub fn new(value: &T) -> Self {
        Self(value)
    }

    /// Reconstructs the reference from the raw pointer.
    ///
    /// # Panics
    /// Panics if it points to uninitialized memory
    ///
    /// # Safety
    /// Ensure that the pointer still points to valid memory.
    /// Neither this method nor this type makes any safety checks or guarantees
    #[must_use]
    pub unsafe fn get(&self) -> &T {
        self.0.as_ref().expect("invalid pointer")
    }
}

impl<T: ?Sized> From<&T> for SendableRawPointer<T> {
    fn from(value: &T) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> Clone for SendableRawPointer<T> {
    fn clone(&self) -> Self {
        *self
    }
}
