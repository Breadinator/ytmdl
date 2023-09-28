use std::str::Chars;

use itertools::PeekNth;

/// Optionally consumes the given sequence from the chars. Returns true if consumed.
///
/// # Example
/// ```
/// let s = "abcdef";
/// let mut chars = itertools::peek_nth(s.chars());
///
/// let found = ytmdl::parsing::consume(&mut chars, "abc");
/// assert_eq!(found, 3);
/// assert_eq!(chars.collect::<String>(), String::from("def"));
/// ```
///
///```
/// let s = "abcdef";
/// let mut chars = itertools::peek_nth(s.chars());
///
/// let found = ytmdl::parsing::consume(&mut chars, "cba");
/// assert_eq!(found, 0);
/// assert_eq!(chars.collect::<String>(), String::from("abcdef"));
/// ```
pub fn consume(chars: &mut PeekNth<Chars<'_>>, sequence: &str) -> usize {
    let mut matches = true;
    for (i, ch) in sequence.chars().enumerate() {
        if chars.peek_nth(i) != Some(&ch) {
            matches = false;
            break;
        }
    }

    if matches {
        // maybe a better way to do this without this loop
        for _ in 0..sequence.len() {
            chars.next();
        }
        sequence.len()
    } else {
        0
    }
}

/// [consume]s the first of the sequences if possible and returns true,
/// else returns false and consumes nothing.
///
/// # Examples
/// ```
/// let url = "https://foo.bar";
/// let mut chars = itertools::peek_nth(url.chars());
///
/// let found = ytmdl::parsing::consume_mutually_exclusive(&mut chars, &["http://", "https://"]);
/// assert_eq!(found, 8);
/// assert_eq!(chars.collect::<String>(), String::from("foo.bar"));
/// ```
///
/// Here is an example of what **NOT** to do.
/// ```
/// let url = "https://foo.bar";
/// let mut chars = itertools::peek_nth(url.chars());
/// let found = ytmdl::parsing::consume_mutually_exclusive(&mut chars, &["http", "https"]);
/// assert_eq!(found, 4);
/// assert_eq!(chars.collect::<String>(), String::from("s://foo.bar")); // bad
/// ```
pub fn consume_mutually_exclusive(chars: &mut PeekNth<Chars<'_>>, sequences: &[&str]) -> usize {
    for sequence in sequences {
        let chars = consume(chars, sequence);
        if chars != 0 {
            return chars;
        }
    }
    0
}
