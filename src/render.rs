//! Utilities to console output
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Truncate the given string to a specified width and pad it with whitespace
/// characters as needed to fill the specified width.
pub fn fit_to_width(s: &str, width: usize) -> String {
    let ellipsis = "…";
    let ellipsis_width = 1; // width of `ellipsis`

    assert!(width >= ellipsis_width);

    let mut out_str = s.to_owned();
    let mut out_str_width = out_str.width();

    if out_str_width > width {
        // Truncate
        out_str.clear();
        out_str_width = 0;
        for ch in s.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if ch_width + out_str_width > width - ellipsis_width {
                break;
            }
            out_str.push(ch);
            out_str_width += ch_width;
        }
        out_str += ellipsis;
        out_str_width += ellipsis_width;
    }

    out_str.extend(std::iter::repeat(' ').take(width - out_str_width));
    out_str
}

#[cfg(tests)]
mod test {
    use super::*;

    fn test_fit_to_width() {
        for &pat in &["", "a", "aaaaaaaaaaa", "Здравствуите!"] {
            let out = fit_to_width(pat, 5);
            assert!(out.width() <= 5);
            if let Some(rest) = out.strip_prefix(pat) {
                assert!(rest.chars().all(|x| x == ' '));
            } else {
                assert!(out.ends_width("..."));
            }
        }
    }
}
