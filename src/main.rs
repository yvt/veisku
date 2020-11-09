use ansi_term::{Color, Style};
use anyhow::{Context, Result};
use clap::Clap;
use std::{convert::Infallible, ffi::OsString, io::Write};

mod cfg;
mod doc;
mod query;
mod render;
mod root;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("v=info")).init();

    let opts: cfg::Opts = Clap::parse();
    log::debug!("opts = {:#?}", opts);

    let root = root::DocRoot::current().context("Failed to get the document root")?;
    log::debug!("root = {:#?}", root);

    match &opts.subcmd {
        cfg::Subcommand::Which(subcmd) => verb_which(&root, subcmd),
        cfg::Subcommand::Open(subcmd) => {
            verb_open(&root, subcmd, default_opener).map(|x| match x {})
        }
        cfg::Subcommand::Show(subcmd) => {
            verb_open(&root, subcmd, default_viewer).map(|x| match x {})
        }
        cfg::Subcommand::Edit(subcmd) => {
            verb_open(&root, subcmd, default_editor).map(|x| match x {})
        }
        cfg::Subcommand::Ls(subcmd) => verb_ls(&root, subcmd),
        cfg::Subcommand::Run(subcmd) => verb_run(&root, subcmd).map(|x| match x {}),
    }
}

fn verb_which(root: &root::DocRoot, sc: &cfg::Query) -> Result<()> {
    let query = query::Query::from_opt(&root.cfg, sc)?;
    let doc = query::select_one(root, &query)?;
    println!("{}", doc.path().display());
    Ok(())
}

fn verb_open(
    root: &root::DocRoot,
    sc: &cfg::Open,
    default_cmd: fn() -> OsString,
) -> Result<Infallible> {
    let query = query::Query::from_opt(&root.cfg, &sc.query)?;
    let doc = query::select_one(root, &query)?;

    let argv = if let Some(cmd) = &sc.cmd {
        let mut cmd: Vec<OsString> = cmd.clone();

        if cmd.iter().any(|x| x == "{}") {
            for e in cmd.iter_mut() {
                if *e == "{}" {
                    *e = doc.path().into();
                }
            }
        } else {
            cmd.push(doc.path().into());
        }

        cmd
    } else {
        vec![default_cmd(), doc.path().into()]
    };

    let mut cmd = std::process::Command::new(&argv[0]);
    cmd.args(&argv[1..]);

    if !sc.preserve_pwd {
        cmd.current_dir(&root.path);
    }

    exec(&mut cmd)
}

fn default_opener() -> OsString {
    if cfg!(target_os = "macos") {
        "open".into()
    } else {
        "xdg-open".into()
    }
}

fn default_viewer() -> OsString {
    if let Some(e) = std::env::var_os("PAGER") {
        e
    } else {
        "less".into()
    }
}

fn default_editor() -> OsString {
    if let Some(e) = std::env::var_os("EDITOR") {
        e
    } else {
        "vi".into()
    }
}

fn verb_ls(root: &root::DocRoot, sc: &cfg::List) -> Result<()> {
    let query = query::Query::from_opt(&root.cfg, &sc.query)?;
    let docs = query::select_all(root, &query);
    let mut out = std::io::BufWriter::new(std::io::stdout());

    #[derive(Debug, thiserror::Error)]
    #[error("An error occurred while enumerating matching documents")]
    struct SearchError;

    #[derive(Debug, thiserror::Error)]
    #[error("An error occurred while writing to the standard output")]
    struct WriteError;

    #[derive(Debug, thiserror::Error)]
    #[error("An error occurred while reading the metadata of {0:?}")]
    struct ReadError(std::path::PathBuf);

    if sc.simple {
        for doc_or_error in docs {
            let doc = doc_or_error.context(SearchError)?;
            writeln!(out, "{}", doc).context(WriteError)?;
        }
    } else if sc.json {
        #[derive(serde::Serialize)]
        struct JsonDoc<'a> {
            path: String,
            meta: &'a serde_yaml::Value,
        }
        writeln!(out, "[").context(WriteError)?;
        for (i, doc_or_error) in docs.enumerate() {
            let mut doc = doc_or_error.context(SearchError)?;
            let path = doc.path().to_owned();
            if i > 0 {
                write!(out, ",\n  ").context(WriteError)?;
            } else {
                write!(out, "  ").context(WriteError)?;
            }
            let json = serde_json::to_string(&JsonDoc {
                path: doc.path().to_string_lossy().into_owned(),
                meta: doc.ensure_meta().with_context(|| ReadError(path.clone()))?,
            })
            .unwrap();
            write!(out, "{}", json).context(WriteError)?;
        }
        writeln!(out, "\n]").context(WriteError)?;
    } else {
        for doc_or_error in docs {
            let mut doc = doc_or_error.context(SearchError)?;
            let path = doc.path().to_owned();
            let name = path.file_stem().unwrap().to_string_lossy();
            let meta = doc.ensure_meta().with_context(|| ReadError(path.clone()))?;

            // Base name
            write!(
                out,
                "{} ",
                // gray
                Color::Fixed(245).paint(render::fit_to_width(&name, 10))
            )
            .context(WriteError)?;

            // Tags
            if let serde_yaml::Value::Sequence(array) = &meta["tags"] {
                for e in array.iter() {
                    if let serde_yaml::Value::String(st) = e {
                        write!(
                            out,
                            "{} ",
                            Style::new()
                                .fg(Color::Green)
                                .on(Color::Fixed(238))
                                .paint(format!(" {} ", st))
                        )
                        .context(WriteError)?;
                    }
                }
            }

            // Title
            let title = if let serde_yaml::Value::String(st) = &meta["title"] {
                &**st
            } else {
                &*name
            };
            write!(out, "{}", title).context(WriteError)?;

            write!(out, "\n").context(WriteError)?;
        }
    }
    Ok(())
}

fn verb_run(root: &root::DocRoot, sc: &cfg::Run) -> Result<Infallible> {
    exec(
        std::process::Command::new(&sc.cmd[0])
            .args(&sc.cmd[1..])
            .current_dir(&root.path),
    )
}

fn exec(cmd: &mut std::process::Command) -> Result<Infallible> {
    match () {
        #[cfg(unix)]
        () => {
            log::debug!("Exec-ing {:?}", cmd);

            use std::os::unix::process::CommandExt;
            Err(cmd.exec()).context("Failed to exec")
        }
        #[cfg(not(unix))]
        () => {
            log::debug!("Spawning {:?}", cmd);
            let child = cmd.spawn().context("Failed to spawn a process")?;
            let result = child
                .wait()
                .context("Failed to wait for the spawned process");
            if result.success() {
                std::process::exit(0);
            } else {
                anyhow::bail!("The child process exited with {}", result.code());
            }
        }
    }
}
