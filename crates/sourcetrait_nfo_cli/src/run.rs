use crate::*;

pub fn run() -> ExitCode {
    let Args { file, profile } = Args::parse();
    let profile = if profile == "default" { None } else { Some(profile.into_owned()) };

    let cfg = match lib::Config::load(profile.as_deref()) {
        Ok(c) => c,
        Err(e) => return fail(&format!("{e}")),
    };

    let meta = match std::fs::metadata(&file) {
        Ok(m) => m,
        Err(e) => return fail(&format!("cannot access {}: {e}", file.display())),
    };
    if meta.is_dir() {
        return fail(&format!(
            "{} is a directory; the tui (nfo) is not built yet",
            file.display()
        ));
    }

    let content = match std::fs::read_to_string(&file) {
        Ok(c) => c,
        Err(e) => return fail(&format!("cannot read {}: {e}", file.display())),
    };

    let rendered = lib::render_markdown_ansi(&content, term_width(), &cfg, lib::NfoSuite::Cli);

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
