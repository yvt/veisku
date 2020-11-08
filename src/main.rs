use anyhow::{Context, Result};
use clap::Clap;
use std::{convert::Infallible, ffi::OsString};

mod cfg;
mod doc;
mod query;
mod root;

fn main() -> Result<()> {
    env_logger::init();
    let opts: cfg::Opts = Clap::parse();
    log::debug!("opts = {:#?}", opts);

    let root = root::DocRoot::current().context("Failed to get the document root")?;
    log::debug!("root = {:#?}", root);

    match &opts.subcmd {
        cfg::Subcommand::Open(subcmd) => {
            verb_open(&root, subcmd, default_viewer).map(|x| match x {})
        }
        cfg::Subcommand::Edit(subcmd) => {
            verb_open(&root, subcmd, default_editor).map(|x| match x {})
        }
        cfg::Subcommand::Ls(subcmd) => verb_ls(&root, subcmd),
        cfg::Subcommand::Run(subcmd) => verb_run(&root, subcmd).map(|x| match x {}),
    }
}

fn verb_open(
    root: &root::DocRoot,
    sc: &cfg::Open,
    default_cmd: fn() -> OsString,
) -> Result<Infallible> {
    let query = query::Query::from_opt(&root.cfg, &sc.query)?;
    let doc = query::select_one(root, &query)?;

    let cmd = if let Some(cmd) = &sc.cmd {
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

    exec(std::process::Command::new(&cmd[0]).args(&cmd[1..]))
}

fn default_viewer() -> OsString {
    if let Some(e) = std::env::var_os("SCREEN") {
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

    for doc_or_error in query::select_all(root, &query) {
        let doc = doc_or_error.context("An error occurred while listing documents")?;
        println!("{}", doc);
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
