use clap::Clap;
use serde::Deserialize;
use std::{collections::HashMap, ffi::OsString, str::FromStr};

// Command-line options
// --------------------------------------------------------------------

/// Personal file-oriented document manager
#[derive(Debug, Clap)]
pub struct Opts {
    /// The command to invoke a pager.
    ///
    /// An empty value disables the use of a pager.
    #[clap(long = "pager", multiple = true, require_delimiter = true)]
    pub pager: Option<Vec<OsString>>,

    #[clap(subcommand)]
    pub subcmd: Option<Subcommand>,

    /// The script to execute (if it doesn't match any builtin subcommand).
    ///
    /// Given a command `NAME ARGS...`, the program will check the following
    /// locations to find the script to run: (1) `v-NAME` in `PATH` (2)
    /// `NAME` in `$root/.veisku/bin`.
    pub cmd: Vec<OsString>,
}

#[derive(Debug, Clap)]
pub enum Subcommand {
    /// Print the path of a document
    Which(Query),
    Edit(Open),
    Open(Open),
    Show(Open),
    Ls(List),
    Run(Run),
}

/// List documents
#[derive(Debug, Clap)]
pub struct List {
    #[clap(flatten)]
    pub query: Query,
    /// Display only full paths
    #[clap(short = '1', long = "simple", group = "mode")]
    pub simple: bool,
    /// Display the result in JSON
    #[clap(short = 'j', long = "json", group = "mode")]
    pub json: bool,
}

/// Open a document
///
/// The search criteria must select exactly one document, or the operation will
/// fail.
///
/// There are variations of this subcommand: edit, open, show. The only
/// differences between them are the default commands they use.
#[derive(Debug, Clap)]
pub struct Open {
    /// The command to open or edit a document.
    ///
    /// If the value contains at least one `{}`, they will be replaced with the
    /// document's path. Otherwise, the path will be appended to the command
    /// line.
    #[clap(
        short = 'c',
        long = "command",
        multiple = true,
        min_values = 1,
        require_delimiter = true
    )]
    pub cmd: Option<Vec<OsString>>,
    #[clap(flatten)]
    pub query: Query,
    /// Preserves the current working directory (does not cd to the document
    /// root).
    #[clap(short = 'p', long = "preserve-pwd")]
    pub preserve_pwd: bool,
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

// Document root configuration
// --------------------------------------------------------------------

/// Document root configuration (`.veisku/config.toml`)
#[derive(Debug, Deserialize)]
pub struct Cfg {
    /// Modifies the document root.
    #[serde(default)]
    pub root: String,

    /// Allows the modification of document metadata, even though it might lose
    /// non-semantic information (such as comments). Currently unused.
    #[serde(default)]
    pub writable: bool,

    /// The patterns of file names to recognize as documents. The patterns are
    /// processed by [`::globwalk`], which supports `gitignore`'s syntax.
    /// The paths are relative to the document root.
    #[serde(default = "files_default")]
    pub files: Vec<String>,

    /// Specifies the text styles applied to various elements
    #[serde(default)]
    pub theme: ThemeCfg,
}

fn files_default() -> Vec<String> {
    ["*.md", "*.mdown", "!*.swp", "!.git/", "!.svn/"]
        .iter()
        .cloned()
        .map(String::from)
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct ThemeCfg {
    /// The mapping between tags and text styles.
    #[serde(default)]
    pub tags: HashMap<String, StyleCfg>,
    #[serde(default = "default_tag_default")]
    pub tag_default: StyleCfg,
}

impl Default for ThemeCfg {
    fn default() -> Self {
        Self {
            tags: HashMap::new(),
            tag_default: default_tag_default(),
        }
    }
}

fn default_tag_default() -> StyleCfg {
    StyleCfg {
        fg: Some(ColorCfg {
            ansi_term_color: ansi_term::Color::Green,
        }),
        bg: Some(ColorCfg {
            ansi_term_color: ansi_term::Color::RGB(64, 64, 64),
        }),
        bold: false,
        italic: false,
    }
}

/// Text style
#[derive(Debug, Default, Deserialize)]
pub struct StyleCfg {
    /// The foreground color
    #[serde(default)]
    fg: Option<ColorCfg>,

    /// The background color
    #[serde(default)]
    bg: Option<ColorCfg>,

    #[serde(default)]
    bold: bool,

    #[serde(default)]
    italic: bool,
}

impl StyleCfg {
    pub fn ansi_term_style(&self) -> ansi_term::Style {
        ansi_term::Style {
            background: self.bg.map(|c| c.ansi_term_color),
            foreground: self.fg.map(|c| c.ansi_term_color),
            is_bold: self.bold,
            is_italic: self.italic,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ColorCfg {
    ansi_term_color: ansi_term::Color,
}

impl<'de> Deserialize<'de> for ColorCfg {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let st = String::deserialize(de)?;

        let ansi_term_color = match &*st {
            "black" => ansi_term::Color::Black,
            "red" => ansi_term::Color::Red,
            "green" => ansi_term::Color::Green,
            "yellow" => ansi_term::Color::Yellow,
            "blue" => ansi_term::Color::Blue,
            "purple" => ansi_term::Color::Purple,
            "cyan" => ansi_term::Color::Cyan,
            "white" => ansi_term::Color::White,
            _ => {
                if let Some([r, g, b]) = parse_hex_color(&st) {
                    ansi_term::Color::RGB(r, g, b)
                } else {
                    return Err(D::Error::custom(format_args!(
                        "invalid hexadecimal color specification: '{}'",
                        st
                    )));
                }
            }
        };

        Ok(Self { ansi_term_color })
    }
}

#[allow(unstable_name_collisions)] // `[_; T]::map` is compatible with `array:Array3::map`
fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    use array::Array3;
    let bytes = s.as_bytes();
    if bytes[0] == b'#' {
        if bytes.len() == 4 {
            if let [Ok(r), Ok(g), Ok(b)] =
                [&s[1..], &s[2..], &s[3..]].map(|x| u8::from_str_radix(&x[..1], 16))
            {
                Some([r * 0x11, g * 0x11, b * 0x11])
            } else {
                None
            }
        } else if bytes.len() == 7 {
            if let [Ok(r), Ok(g), Ok(b)] =
                [&s[1..], &s[3..], &s[5..]].map(|x| u8::from_str_radix(&x[..2], 16))
            {
                Some([r, g, b])
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}
