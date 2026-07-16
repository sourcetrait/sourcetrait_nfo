/// Which UI component the library is rendering for. lib renders one semantic
/// `Document` many ways; the component kind drives target-specific policy -
/// notably which links are `Unable` (see `marks_unable`), and later which get
/// OSC 8 hyperlinks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentKind {
    /// `nfout` - the read-only terminal pretty-printer.
    Cli,
    /// `nfo` - the interactive terminal UI.
    Tui,
    /// The iced desktop UI.
    Gui,
    /// The servo-backed web UI.
    Wui,
    /// The axum HTTP server.
    Srv,
    /// The mdbook preprocessor.
    Mdbook,
}

impl ComponentKind {
    /// Whether this component must treat a link to `dest` as `Unable` - one it
    /// cannot (or will not) emit as a live OSC 8 hyperlink, so the renderer
    /// surfaces the destination inline instead. A non-interactive terminal
    /// cannot trap a local file link (and ctrl-click would spawn an ssh
    /// session), so the cli marks local links Unable; interactive components do
    /// not have that problem.
    pub(crate) fn marks_unable(&self, dest: &str) -> bool {
        match self {
            ComponentKind::Cli => is_local(dest),
            _ => false,
        }
    }
}

/// Whether a link destination is a local file path rather than a remote URL:
/// a dest with a URI scheme (`http://`, `https://`, `mailto:`, ...) is remote;
/// everything else (relative/absolute paths, `#fragments`) is local.
fn is_local(dest: &str) -> bool {
    !has_scheme(dest)
}

/// Whether `dest` begins with a URI scheme (`scheme:`), per a permissive
/// RFC-3986-ish check: an ASCII letter, then letters/digits/`+`/`-`/`.` up to
/// the first colon.
fn has_scheme(dest: &str) -> bool {
    let Some(colon) = dest.find(':') else {
        return false;
    };
    let scheme = &dest[..colon];
    !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}
