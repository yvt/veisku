//! Utilities for console output
use std::{
    io::{BufWriter, Write},
    process::{Child, Stdio},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::cfg::Opts;

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

pub struct Pager {
    /// The `Child` object representing the process of a pager. `None` if the
    /// output is directly written to the standard output.
    child: Option<AutokillChild>,
    writer: BufWriter<Box<dyn Write>>,
}

impl Pager {
    pub fn new(opts: &Opts) -> Self {
        let pager = opts.pager.clone().unwrap_or_else(|| {
            if console::Term::stdout().features().is_attended() {
                log::debug!(
                    "The pager is not specified; using the default pager because \
                        stdout connects to an attended terminal"
                );
                vec!["less".into(), "--RAW-CONTROL-CHARS".into()]
            } else {
                log::debug!(
                    "The pager is not specified; not using a pager because \
                        stdout doesn't connect to an attended terminal"
                );
                vec![]
            }
        });

        log::debug!("pager = {:?}", pager);

        if pager.is_empty() || pager[0].is_empty() {
            log::debug!("The pager is not specified; outputting to stdout");
            return Self::pagerless();
        }

        let mut child = match std::process::Command::new(&pager[0])
            .args(&pager[1..])
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                log::warn!(
                    "Failed to spawn the process of a pager; outputting to stdout: {:?}",
                    e
                );
                return Self::pagerless();
            }
        };

        let writer = BufWriter::new(Box::new(child.stdin.take().unwrap()) as _);

        Self {
            child: Some(AutokillChild(child)),
            writer,
        }
    }

    /// Construct `Self` that directs the output to the standard output.
    fn pagerless() -> Self {
        Self {
            child: None,
            writer: BufWriter::new(Box::new(std::io::stdout())),
        }
    }

    /// Mark the end of output and wait for the pager to exit.
    pub fn finish(mut self) -> std::io::Result<()> {
        // Close the writer
        self.writer.flush()?;
        drop(self.writer);

        // Wait until the pager exits
        if let Some(mut child) = self.child {
            let _ = child.0.wait();
        }

        Ok(())
    }
}

impl std::io::Write for Pager {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

struct AutokillChild(Child);

impl Drop for AutokillChild {
    fn drop(&mut self) {
        if let Err(e) = self.0.kill() {
            if e.kind() == std::io::ErrorKind::InvalidInput {
                // It's already dead
                return;
            }
            log::warn!("Failed to kill a child process: {:?}", e);
        }
    }
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
