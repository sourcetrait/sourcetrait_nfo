//! Integration tests over the public render + config surface.

use sourcetrait_nfo_lib::{render_markdown_ansi, ComponentKind, Config};

/// Strip ANSI SGR sequences AND OSC 8 hyperlinks so we can assert on text.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // OSC (e.g. OSC 8 hyperlink): consume through ST (ESC \) or BEL.
            Some(']') => {
                while let Some(c2) = chars.next() {
                    if c2 == '\u{7}' {
                        break;
                    }
                    if c2 == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            // CSI/SGR: consume through the alphabetic final byte.
            _ => {
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        }
    }
    out
}

/// `[link]` is a used reference; `[other]` is defined but never referenced.
const DOC: &str = "This [link] gets referenced.\n\n[link]: foo/bar.md\n[other]: bar/foo.md\n";

fn render(config_toml: &str, md: &str) -> String {
    let cfg = Config::from_toml(config_toml).expect("valid config");
    plain(&render_markdown_ansi(md, 80, &cfg, ComponentKind::Cli))
}

#[test]
fn reference_unused_lists_only_orphans() {
    let out = render("[render]\nreference = \"unused\"\n", DOC);
    assert!(out.contains("[other]: bar/foo.md"), "orphan listed:\n{out}");
    assert!(!out.contains("[link]: foo/bar.md"), "used def hidden:\n{out}");
}

#[test]
fn reference_always_lists_all() {
    let out = render("[render]\nreference = \"always\"\n", DOC);
    assert!(out.contains("[link]: foo/bar.md"), "used def listed:\n{out}");
    assert!(out.contains("[other]: bar/foo.md"), "orphan listed:\n{out}");
}

#[test]
fn reference_never_lists_none() {
    let out = render("[render]\nreference = \"never\"\n", DOC);
    assert!(!out.contains("[other]:"), "no references:\n{out}");
    assert!(!out.contains("[link]:"), "no references:\n{out}");
}

#[test]
fn default_theme_shows_heading_markup() {
    // The default theme sets `all.md.render = true`, so headings render their
    // leading `#` markup (glow-ish). Markup is a theme concern, not config.
    let out = render("", "# Title\n");
    assert!(out.contains("# Title"), "markup shown by default:\n{out}");
}

#[test]
fn soft_break_joins_lines_with_a_space() {
    // A bare newline is a soft break -> a space (CommonMark); a hard break needs
    // a trailing "  \n". "line 1\nline2" is one paragraph, not two lines.
    let out = render("", "line 1\nline2\n");
    assert!(out.contains("line 1 line2"), "soft break renders as a space:\n{out}");
}

#[test]
fn wrap_band_wraps_after_min_on_the_next_token() {
    // min=10, max=20: a line fills until it exceeds min (10), then the next
    // token wraps. "aaaa bbbb cccc" reaches 14 (> 10), so "dddd" wraps.
    let out = render("[wrap]\nmin = 10\nmax = 20\n", "aaaa bbbb cccc dddd\n");
    assert!(out.contains("aaaa bbbb cccc"), "first line fills past min:\n{out}");
    assert!(!out.contains("cccc dddd"), "wrapped before dddd:\n{out}");
}

#[test]
fn code_block_renders_fence_by_default() {
    // code.md.render = true by default -> the ```lang fence shows; the body is
    // not extra-indented (only the body left margin).
    let out = render("", "```rust\nfn main() {}\n```\n");
    assert!(out.contains("```rust"), "opening fence with lang:\n{out}");
    assert!(out.contains("fn main() {}"), "code body:\n{out}");
}

#[test]
fn inline_markup_hidden_by_default() {
    // Default theme: all.md.render = false, so inline markers are hidden - the
    // text renders styled without the `**`/`*`/`` ` ``/`~~`. Headings and the
    // unable-link dest are the exceptions (md.render = true).
    let out = render("", "a **b** c *d* e `f` g ~~h~~ i\n");
    assert!(out.contains("a b c d e f g h i"), "markers stripped:\n{out}");
    assert!(!out.contains("**"), "no strong markers:\n{out}");
    assert!(!out.contains("~~"), "no strikethrough markers:\n{out}");
}

#[test]
fn cli_marks_local_links_unable_and_inlines_the_dest() {
    // A local file link cannot be an OSC 8 hyperlink in the cli, so its dest is
    // surfaced inline after the text (indent [1, 0] -> one leading space).
    let out = render("", "See [the guide](docs/guide.md).\n");
    assert!(out.contains("the guide (docs/guide.md)"), "local dest inlined:\n{out}");
}

#[test]
fn cli_remote_link_text_only_no_inline_dest() {
    // A remote link is able -> an OSC 8 hyperlink (stripped here); the plain text
    // is just the link text, with no inline `(dest)`.
    let out = render("", "See [the site](https://example.com).\n");
    assert!(out.contains("the site"), "link text kept:\n{out}");
    assert!(!out.contains("(https://example.com)"), "remote dest not inlined:\n{out}");
}

#[test]
fn cli_remote_link_emits_osc8_hyperlink() {
    // Able (remote) link -> its text is wrapped in an OSC 8 hyperlink to dest.
    let cfg = Config::from_toml("").expect("valid config");
    let raw = render_markdown_ansi("See [the site](https://example.com).\n", 80, &cfg, ComponentKind::Cli);
    assert!(raw.contains("\u{1b}]8;;https://example.com\u{1b}\\"), "osc8 open:\n{raw:?}");
    assert!(raw.contains("\u{1b}]8;;\u{1b}\\"), "osc8 close:\n{raw:?}");
}

#[test]
fn cli_local_link_no_osc8() {
    // A local link is Unable -> no OSC 8; the dest is surfaced inline instead.
    let cfg = Config::from_toml("").expect("valid config");
    let raw = render_markdown_ansi("See [the guide](docs/guide.md).\n", 80, &cfg, ComponentKind::Cli);
    assert!(!raw.contains("\u{1b}]8;;"), "no osc8 for a local (Unable) link:\n{raw:?}");
}

#[test]
fn empty_config_keeps_defaults() {
    // Empty user config -> all fields fall back to the embedded defaults
    // (reference = unused).
    let out = render("", DOC);
    assert!(out.contains("[other]: bar/foo.md"), "default unused:\n{out}");
    assert!(!out.contains("[link]: foo/bar.md"), "default unused:\n{out}");
}
