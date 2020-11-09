//! Document metadata parsing
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::{
    fmt,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
};

/// Represents a reference to a document. Metadata is read as needed (lazy
/// loading).
pub struct DocRead {
    path: PathBuf,
    meta: Option<Value>,
}

impl DocRead {
    pub fn new(path: PathBuf) -> Self {
        Self { path, meta: None }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ensure_meta(&mut self) -> Result<&Value> {
        if self.meta.is_none() {
            log::trace!("Reading the metadata of {:?}", self.path);

            let file = std::fs::File::open(&self.path)
                .with_context(|| format!("Failed to open {:?}", self.path))?;

            self.meta = Some(
                read_md_preamble(file)
                    .with_context(|| format!("Failed to read metadata from {:?}", self.path))?
                    .unwrap_or(Value::Null),
            );
        }
        Ok(self.meta.as_ref().unwrap())
    }
}

impl fmt::Display for DocRead {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.path().display())
    }
}

fn read_md_preamble(mut file: impl Read) -> Result<Option<Value>> {
    // We need to find a preamble in the file stream. A preamble is supposed
    // to look like the following:
    //
    //     ---
    //     key1: value1
    //     key2: value2
    //     ---
    //     <file body>
    //
    let separators: &[[&[u8]; 2]] = &[
        [b"---\r\n", b"\r\n---\r\n"],
        [b"---\n", b"\n---\n"],
        [b"---\r", b"\r---\r"],
    ];
    let mut buf = [0u8; 1 << 12];
    let mut pre_bytes: Vec<u8> = Vec::new();

    // Find the first separator
    match file.read_exact(&mut buf[..5]) {
        Ok(()) => {}
        // If we encountered EOF at this point, the file is clearly too short to
        // contain the preamble.
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e).context("Failed to read the file"),
    }

    let sep2 = if let Some([sep1, sep2]) = separators
        .iter()
        .find(|[sep1, _]| buf[..5].starts_with(sep1))
    {
        // Found the first separator. `buf[..5]` might the first few bytes of
        // the preamble body if `separator` is shorter than `buf[..5]`.
        pre_bytes.extend_from_slice(&buf[sep1.len()..5]);
        sep2
    } else {
        // Did not find the first separator.
        return Ok(None);
    };

    // Munch the preamble body until we find the second separator
    loop {
        let num_bytes_read = file.read(&mut buf).context("Failed to read the file")?;

        if num_bytes_read == 0 {
            // We did not find the second separator. Maybe what we thought to be
            // a preamble wasn't actually a preamble.
            log::warn!("Encountered EOF while reading the preamble");
            return Ok(None);
        }

        let search_start = pre_bytes.len().saturating_sub(sep2.len() - 1);
        pre_bytes.extend_from_slice(&buf[..num_bytes_read]);

        // Look for the second separator
        if let Some((i, _)) = pre_bytes[search_start..]
            .windows(sep2.len())
            .enumerate()
            .find(|(_, window)| window == sep2)
        {
            // Found the second separator at `pre_bytes[search_start + i..][..sep2.len()]`
            pre_bytes.truncate(search_start + i);
            break;
        }
    }

    drop(file);

    // Interpret the preamble as UTF-8
    let pre_str =
        std::str::from_utf8(&pre_bytes).context("Failed to decdoe the preamble as UTF-8")?;
    log::trace!("pre_str = {:?}", pre_str);

    // Now, parse the preamble.
    let yaml_value =
        serde_yaml::from_str(pre_str).context("Failed to parse the preamble as YAML")?;
    Ok(Some(yaml_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_md_preamble() {
        assert!(read_md_preamble(&b"no preamble"[..]).unwrap().is_none());

        read_md_preamble(&b"---\nval1: key1\n---\nbody"[..])
            .unwrap()
            .unwrap();
    }
}
