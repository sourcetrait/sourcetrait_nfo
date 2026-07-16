use crate::*;

/// `nfout` command-line arguments. The cli is thin: it routes input and prints;
/// all rendering lives in `lib`.
#[derive(Parser, Debug)]
#[command(name = "nfout", about = "Pretty-print markdown to the terminal (read-only).")]
pub(crate) struct Args {
    /// Markdown file to render. Passing a directory will dispatch to the tui
    /// (`nfo`) once it exists; not yet wired. Optional only for `-c defaults`.
    pub(crate) path: Option<std::path::PathBuf>,

    /// Config profile: load
    /// `$XDG_CONFIG_HOME/sourcetrait/nfo/profile/<name>/` instead of the
    /// default set. The reserved name `defaults` regenerates
    /// `profile/defaults/` from the built-in defaults (a viewable template).
    #[arg(short = 'c', long = "config")]
    pub(crate) config: Option<String>,
}
