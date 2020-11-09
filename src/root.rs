//! Document root discovery and configuration retrieval
use anyhow::{Context, Error, Result};
use either::{Left, Right};
use std::path::{Path, PathBuf};

use crate::{cfg::Cfg, doc::DocRead};

/// Contains the configuration data of a document root.
#[derive(Debug)]
pub struct DocRoot {
    pub path: PathBuf,
    pub cfg: Cfg,
}

impl DocRoot {
    /// Locate the doocument root based on the current working directory and
    /// return the corresponding `DocRoot` object.
    pub fn current() -> Result<Self> {
        // Locate the document root
        let current_dir =
            std::env::current_dir().context("Failed to determine the current directory")?;
        let mut doc_root_path: &Path = &current_dir;
        {
            let mut dir: &Path = &current_dir;
            while {
                log::trace!("Checking if {:?} contains a configuration directory", dir);
                let cfg_dir_path = cfg_dir_path_for_doc_root_path(&dir);
                if cfg_dir_path.is_dir() {
                    log::trace!(
                        "Found the directory {:?}; using {:?} as the document root",
                        cfg_dir_path,
                        dir
                    );
                    doc_root_path = dir;
                    false
                } else if let Some(next_dir) = dir.parent() {
                    dir = next_dir;
                    true
                } else {
                    log::debug!(
                        "Could not locate a configuration directory; using {:?} as the document root",
                        current_dir
                    );
                    false
                }
            } {}
        }

        // Read the configuration
        let cfg_path = cfg_file_path_for_doc_root_path(doc_root_path);
        let cfg_toml = if cfg_path.exists() {
            log::trace!("Reading configuration from {:?}", cfg_path);
            std::fs::read_to_string(&cfg_path).context("Failed to read `config.toml`")?
        } else {
            log::trace!(
                "{:?} doesn't exist; using the default configuration",
                cfg_path
            );
            String::new()
        };
        let cfg: Cfg = toml::de::from_str(&cfg_toml).context("Failed to parse `config.toml`")?;

        // Decide the final document root
        let doc_root_path = doc_root_path.join(&cfg.root);
        let doc_root_path = doc_root_path.canonicalize().with_context(|| {
            format!(
                "Failed to canonicalize the document root {:?}",
                doc_root_path
            )
        })?;

        Ok(DocRoot {
            path: doc_root_path,
            cfg,
        })
    }

    pub fn script_dir_path(&self) -> PathBuf {
        self.path.join("bin")
    }
}

/// Get the configuration directory path for the specified document root.
fn cfg_dir_path_for_doc_root_path(doc_root_path: &Path) -> PathBuf {
    doc_root_path.join(".veisku")
}

/// Get the configuration path for the specified document root.
fn cfg_file_path_for_doc_root_path(doc_root_path: &Path) -> PathBuf {
    doc_root_path.join(".veisku/config.toml")
}

impl DocRoot {
    /// Return an iterator over the document files in the document root.
    pub fn doc_files(&self) -> impl Iterator<Item = Result<globwalk::DirEntry, Error>> {
        match globwalk::GlobWalkerBuilder::from_patterns(&self.path, &self.cfg.files)
            .follow_links(true)
            .build()
        {
            Ok(it) => Left(it.map(|e| e.map_err(Into::into))),
            Err(e) => Right(std::iter::once(Err(e.into()))),
        }
    }

    /// Return an iterator over the `DocRead` objects representing the document
    /// files in the document root.
    pub fn docs(&self) -> impl Iterator<Item = Result<DocRead, Error>> {
        self.doc_files()
            .map(|entry_or_err| entry_or_err.map(|entry| DocRead::new(entry.into_path())))
    }
}
