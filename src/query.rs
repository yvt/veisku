use crate::{
    cfg::{Cfg, Criterion, SimpleCriterion},
    doc::DocRead,
    root::DocRoot,
};
use anyhow::{Context, Error, Result};
use std::fmt;
use yaml_rust::Yaml;

/// Compiled document query
#[derive(Debug)]
pub struct Query {
    smart_name: Option<String>,
    matchers: Vec<Box<dyn Matcher>>,
}

trait Matcher: std::fmt::Debug + Send + Sync {
    fn matches(&self, doc: &mut DocRead) -> Result<bool>;
}

impl Query {
    /// Construct `Query` from command-line options.
    pub fn from_opt(_cfg: &Cfg, in_query: &crate::cfg::Query) -> Result<Self> {
        let mut query = Query {
            smart_name: None,
            matchers: Vec::new(),
        };

        // TODO: query preset
        if in_query.preset != "default" && in_query.preset != "" {
            anyhow::bail!("Unknown query preset: '{}'", in_query.preset);
        }

        for criterion in in_query.criteria.iter() {
            match criterion {
                Criterion::NameSmart(smart_name) => {
                    if query.smart_name.is_some() {
                        anyhow::bail!("Smart name search criteria can only appear once");
                    }
                    query.smart_name = Some(smart_name.clone());
                }
                Criterion::Simple {
                    negate,
                    simple_criterion,
                } => {
                    let mut matcher: Box<dyn Matcher> = match simple_criterion {
                        SimpleCriterion::NameRegex(regex) => Box::new(NameRegex {
                            regex: regex::Regex::new(&regex).with_context(|| {
                                format!("Failed to comple the regex '{}'", regex)
                            })?,
                        }),
                        SimpleCriterion::MetaEq(key, value) => Box::new(Meta {
                            key: key.clone(),
                            op: MetaOp::Eq(value.clone()),
                        }),
                        SimpleCriterion::MetaRegex(key, regex) => Box::new(Meta {
                            key: key.clone(),
                            op: MetaOp::Regex(regex::Regex::new(&regex).with_context(|| {
                                format!("Failed to comple the regex '{}'", regex)
                            })?),
                        }),
                    };

                    if *negate {
                        matcher = Box::new(Negate(matcher));
                    }

                    query.matchers.push(matcher);
                }
            }
        }

        log::debug!("compiled query = {:?}", query);

        Ok(query)
    }
}

#[derive(Debug)]
struct Always;

impl Matcher for Always {
    fn matches(&self, _doc: &mut DocRead) -> Result<bool> {
        Ok(true)
    }
}

#[derive(Debug)]
struct Never;

impl Matcher for Never {
    fn matches(&self, _doc: &mut DocRead) -> Result<bool> {
        Ok(false)
    }
}

#[derive(Debug)]
struct Negate(Box<dyn Matcher>);

impl Matcher for Negate {
    fn matches(&self, doc: &mut DocRead) -> Result<bool> {
        Ok(!self.0.matches(doc)?)
    }
}

/// The matcher that applies regex on document names.
#[derive(Debug)]
struct NameRegex {
    regex: regex::Regex,
}

impl Matcher for NameRegex {
    fn matches(&self, doc: &mut DocRead) -> Result<bool> {
        if let Some(stem) = doc.path().file_stem().and_then(|s| s.to_str()) {
            Ok(self.regex.is_match(stem))
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug)]
struct SmartNameExact<'a> {
    pattern: &'a str,
}

impl Matcher for SmartNameExact<'_> {
    fn matches(&self, doc: &mut DocRead) -> Result<bool> {
        if let Some(stem) = doc.path().file_stem() {
            Ok(stem == self.pattern)
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug)]
struct SmartNamePrefix<'a> {
    pattern: &'a str,
}

impl Matcher for SmartNamePrefix<'_> {
    fn matches(&self, doc: &mut DocRead) -> Result<bool> {
        if let Some(stem) = doc.path().file_stem().and_then(|s| s.to_str()) {
            Ok(stem.starts_with(self.pattern))
        } else {
            Ok(false)
        }
    }
}

/// The matcher that tries to equate field values.
#[derive(Debug)]
struct Meta {
    key: String,
    op: MetaOp,
}

#[derive(Debug)]
enum MetaOp {
    Eq(String),
    Regex(regex::Regex),
}

impl Matcher for Meta {
    fn matches(&self, doc: &mut DocRead) -> Result<bool> {
        let meta_path;
        let meta = if self.key == "path" {
            meta_path = Yaml::String(doc.path().to_string_lossy().into_owned());
            &meta_path
        } else {
            &doc.ensure_meta()?[&*self.key]
        };
        match self.op.matches(meta) {
            Some(x) => Ok(x),
            None => {
                log::warn!(
                    "The field '{}' of document '{}' contains an object of an \
                    uncomparable type; can't apply Meta matcher",
                    self.key,
                    doc
                );
                Ok(false)
            }
        }
    }
}

impl MetaOp {
    fn matches(&self, yaml: &Yaml) -> Option<bool> {
        match yaml {
            Yaml::String(st) => Some(match self {
                Self::Eq(rhs) => **st == *rhs,
                Self::Regex(regex) => regex.is_match(st),
            }),
            Yaml::Array(array) => {
                if array.is_empty() {
                    Some(false)
                } else {
                    array
                        .iter()
                        .map(|e| self.matches(e))
                        // Take the maximum value based on the ordering:
                        // `Some(true) > Some(false) > None`, producing the following
                        // properties:
                        //
                        //  - If any element matches, `yaml_eq` returns `Some(true)`
                        //
                        //  - If the above is not the case but at least one element
                        //    is comparable, `yaml_eq` returns `Some(false)`.
                        //
                        //  - If none of the elements are comparable, `yaml_eq`
                        //    returns `None`.
                        //
                        .fold(None, |acc, x| match (acc, x) {
                            (Some(true), _) | (_, Some(true)) => Some(true),
                            (Some(false), _) | (_, Some(false)) => Some(false),
                            (None, None) => None,
                        })
                }
            }
            Yaml::Null => Some(false),
            _ => {
                // Uncomparable
                None
            }
        }
    }
}

pub fn select_all<'a>(
    root: &DocRoot,
    query: &'a Query,
) -> impl Iterator<Item = Result<DocRead, Error>> + 'a {
    for phase in 0..2 {
        let smart_name_matcher: Box<dyn Matcher> = match (&query.smart_name, phase) {
            (Some(smart_name), 0) => Box::new(SmartNameExact {
                pattern: smart_name,
            }),
            (Some(smart_name), 1) => Box::new(SmartNamePrefix {
                pattern: smart_name,
            }),
            (None, 0) => Box::new(Always),
            (None, _) => Box::new(Never),
            (_, 2..=u32::MAX) => unreachable!(),
        };

        fn apply_matcher(
            acc: Option<Result<DocRead, Error>>,
            matcher: &dyn Matcher,
        ) -> Option<Result<DocRead, Error>> {
            match acc {
                Some(Ok(mut doc)) => match matcher.matches(&mut doc) {
                    Ok(true) => Some(Ok(doc)),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                },
                x => x,
            }
        }

        let mut iterator = root
            .docs()
            .filter_map(move |doc_or_err| {
                query.matchers.iter().fold(
                    apply_matcher(Some(doc_or_err), &*smart_name_matcher),
                    |acc, matcher| apply_matcher(acc, &**matcher),
                )
            })
            .peekable();

        if iterator.peek().is_some() || phase == 1 {
            return iterator;
        }

        // If the iterator returned no element, proceed to the next phase
    }

    unreachable!()
}

pub enum SelectOneError {
    Empty,
    Ambiguous {
        candidates: Vec<DocRead>,
        truncated: bool,
    },
    Misc(Error),
}

impl fmt::Display for SelectOneError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Did not match anything"),
            Self::Ambiguous {
                candidates,
                truncated,
            } => {
                write!(f, "Ambigous document selection. Candidates:")?;
                for doc in candidates.iter() {
                    write!(f, "\n - {}", doc)?;
                }
                if *truncated {
                    write!(f, "\n - (truncated)")?;
                }
                Ok(())
            }
            Self::Misc(e) => write!(f, "{}", e),
        }
    }
}

impl fmt::Debug for SelectOneError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Did not match anything"),
            Self::Ambiguous { .. } => write!(f, "{}", self),
            Self::Misc(e) => write!(f, "{:?}", e),
        }
    }
}

impl std::error::Error for SelectOneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Misc(e) = self {
            e.source()
        } else {
            None
        }
    }
}

pub fn select_one<'a>(root: &DocRoot, query: &'a Query) -> Result<DocRead, SelectOneError> {
    let mut it = select_all(root, query);

    // Get the first result
    let first = match it.next() {
        Some(Ok(x)) => x,
        Some(Err(e)) => return Err(SelectOneError::Misc(e)),
        None => return Err(SelectOneError::Empty),
    };

    // Check if the result is singular
    let second = match it.next() {
        Some(Ok(x)) => x,
        Some(Err(e)) => return Err(SelectOneError::Misc(e)),
        // The result is singular, so return it.
        None => return Ok(first),
    };

    // Found the second result. Report an error. But first collect a few more
    // results to present to the user.
    let num_candidates_to_display = 10;
    let mut candidates = vec![first, second];
    for _ in 0..num_candidates_to_display - 1 {
        match it.next() {
            Some(Ok(x)) => candidates.push(x),
            Some(Err(e)) => return Err(SelectOneError::Misc(e)),
            None => break,
        }
    }

    assert!(candidates.len() <= num_candidates_to_display + 1);

    // If there are more than `num_candidates_to_display` candidates, indicate
    // that the list has been truncated.
    let truncated = candidates.len() == num_candidates_to_display + 1;
    if truncated {
        candidates.pop().unwrap();
    }

    Err(SelectOneError::Ambiguous {
        candidates,
        truncated,
    })
}
