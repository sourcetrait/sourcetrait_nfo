use crate::*;

/// Parse args, load the config (a profile or the default set), render the
/// markdown file to the terminal (or report a directory pending the tui), and
/// print. Returns the process exit code.
pub fn run() -> ExitCode {
    let Args { path, config } = Args::parse();
    let profile = config.as_deref();

    // Reserved profile `defaults`: regenerate the templates first.
    if profile == Some("defaults") {
        match lib::Config::write_defaults() {
            Ok(paths) => {
                if path.is_none() {
                    for p in &paths {
                        println!("nfout: wrote {}", p.display());
                    }
                    return ExitCode::SUCCESS;
                }
            }
            Err(e) => return fail(&format!("{e}")),
        }
    }

    let path = match path {
        Some(p) => p,
        None => return fail("no markdown file given"),
    };

    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) => return fail(&format!("cannot access {}: {e}", path.display())),
    };
    if meta.is_dir() {
        return fail(&format!(
            "{} is a directory; the tui (nfo) is not built yet",
            path.display()
        ));
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return fail(&format!("cannot read {}: {e}", path.display())),
    };

    let load_profile = match profile {
        Some("defaults") => None,
        other => other,
    };
    let cfg = match lib::Config::load(load_profile) {
        Ok(c) => c,
        Err(e) => return fail(&format!("{e}")),
    };

    let rendered = lib::render_markdown_ansi(&content, term_width(), &cfg, lib::ComponentKind::Cli);

    let mut out = io::stdout().lock();
    if out.write_all(rendered.as_bytes()).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("nfout: {msg}");
    ExitCode::FAILURE
}

/// The terminal width in columns, or 80 when stdout is not a tty.
fn term_width() -> usize {
    match terminal_size::terminal_size() {
        Some((terminal_size::Width(w), _)) => w as usize,
        None => 80,
    }
}
