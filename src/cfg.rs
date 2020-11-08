use clap::Clap;
use serde::Deserialize;
use std::{ffi::OsString, str::FromStr};

/// Personal file-oriented document manager
#[derive(Debug, Clap)]
pub struct Opts {
    #[clap(subcommand)]
    pub subcmd: Subcommand,
}

#[derive(Debug, Clap)]
pub enum Subcommand {
    Edit(Open),
    Open(Open),
    Ls(List),
    Run(Run),
}

/// List documents
#[derive(Debug, Clap)]
pub struct List {
    #[clap(flatten)]
    pub query: Query,
}

/// Open a document
///
/// The search criteria must select exactly one document, or the operation will
/// fail.
#[derive(Debug, Clap)]
pub struct Open {
    /// The command to open or edit a document.
    ///
    /// If the value contains at least one `{}`, they will be replaced with the
    /// document's path. Otherwise, the path will be appended to the command
    /// line.
    #[clap(short = 'c', long = "command", multiple = true, min_values = 1)]
    pub cmd: Option<Vec<OsString>>,
    #[clap(flatten)]
    pub query: Query,
}

/// Execute a command in the document root
#[derive(Debug, Clap)]
pub struct Run {
    /// The command to execute.
    #[clap(required = true)]
    pub cmd: Vec<OsString>,
}

#[derive(Debug, Clap)]
pub struct Query {
    /// Specifies a pre-defined filter. An empty string disables the default
    /// filter.
    #[clap(short = 'f', long = "filter", default_value = "default")]
    pub preset: String,

    /// Conjunctive search criteria
    ///
    ///  - `STRING` performs a smart name search (can be used only once in a
    ///    single query). First, it looks for documents with an exactly matching
    ///    base name. If none was found, then it looks for documents whose base
    ///    names start with `STRING`.
    ///
    ///  - `/REGEX/` matches documents whose base names match the specified
    ///    regex.
    ///
    ///  - `KEY:VALUE` matches a metadata field having the name `KEY` and value
    ///    `VALUE`.
    ///
    ///      - `path:VALUE` matches the full path of a document.
    ///
    ///  - `KEY:/VALUE/` matches a metadata field having the name `KEY` and
    ///    a value matching the regex `VALUE`.
    ///
    ///  - The `!` prefix negates the criterion. Illegal for a smart search.
    ///
    /// # Unimplemented syntax
    ///
    ///  - `contents:TEXT` - please use ripgrep for now
    ///
    ///  - `KEY:<VALUE`, `KEY:>VALUE`, `KEY:<=VALUE`, `KEY:>=VALUE`, `KEY:<>VALUE`
    ///
    ///  - `=EXPRESSION`
    ///
    pub criteria: Vec<Criterion>,
}

#[derive(Debug)]
pub enum Criterion {
    NameSmart(String),
    Simple {
        negate: bool,
        simple_criterion: SimpleCriterion,
    },
}

#[derive(Debug)]
pub enum SimpleCriterion {
    NameRegex(String),
    MetaEq(String, String),
    MetaRegex(String, String),
}

impl FromStr for Criterion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (negate, s) = if let Some(s) = s.strip_prefix("!") {
            (true, s)
        } else {
            (false, s)
        };

        if let Some(s) = s.strip_prefix("/").and_then(|s| s.strip_suffix("/")) {
            Ok(Self::Simple {
                negate,
                simple_criterion: SimpleCriterion::NameRegex(s.to_owned()),
            })
        } else if s.starts_with("=") {
            Err("`=EXPRESSION` syntax is not implemented")
        } else if let Some(i) = s.find(":") {
            let key = &s[..i];
            let value = &s[i + 1..];
            if value.starts_with("<") || value.starts_with(">") {
                Err("Unimplemented syntax")
            } else if let Some(s) = value.strip_prefix("/").and_then(|s| s.strip_suffix("/")) {
                Ok(Self::Simple {
                    negate,
                    simple_criterion: SimpleCriterion::MetaRegex(key.to_owned(), s.to_owned()),
                })
            } else {
                Ok(Self::Simple {
                    negate,
                    simple_criterion: SimpleCriterion::MetaEq(key.to_owned(), value.to_owned()),
                })
            }
        } else {
            // Smart name search
            if negate {
                Err("Smart name search cannot be used with negation")
            } else {
                Ok(Self::NameSmart(s.to_owned()))
            }
        }
    }
}

/// Document root configuration (`.veisku/config.toml`)
#[derive(Debug, Deserialize)]
pub struct Cfg {
    /// Modifies the document root.
    #[serde(default)]
    pub root: String,

    /// The patterns of file names to recognize as documents. The patterns are
    /// processed by [`::globwalk`], which supports `gitignore`'s syntax.
    /// The paths are relative to the document root.
    #[serde(default = "files_default")]
    pub files: Vec<String>,
}

fn files_default() -> Vec<String> {
    ["*.md", "*.mdown", "!*.swp", "!.git/", "!.svn/"]
        .iter()
        .cloned()
        .map(String::from)
        .collect()
}
