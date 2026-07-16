use crate::component::ComponentKind;

/// The embedded default theme - the built-in glow-ish palette + the template
/// `nfout -c defaults` writes to `theme/defaults.toml`.
pub(crate) const DEFAULT_THEME_TOML: &str = include_str!("default_theme.toml");

/// An 8-bit-per-channel RGB color (internal render form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Color {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

/// A resolved text style: optional fg/bg plus the emphasis attributes the
/// terminal serializes to ANSI. Mirrors a theme term's `.fg`/`.bg`/`.style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Style {
    pub(crate) fg: Option<Color>,
    pub(crate) bg: Option<Color>,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
}

impl Style {
    /// Overlay `inner` onto `self`: attributes accumulate, a set fg/bg on
    /// `inner` wins. Combines the term cascade and nested inline styling.
    pub(crate) fn over(self, inner: Style) -> Style {
        Style {
            fg: inner.fg.or(self.fg),
            bg: inner.bg.or(self.bg),
            bold: self.bold || inner.bold,
            italic: self.italic || inner.italic,
            underline: self.underline || inner.underline,
            strikethrough: self.strikethrough || inner.strikethrough,
        }
    }

    pub(crate) fn is_plain(&self) -> bool {
        self.fg.is_none()
            && self.bg.is_none()
            && !self.bold
            && !self.italic
            && !self.underline
            && !self.strikethrough
    }
}

/// A margin: `top`/`bottom` are line counts, `left`/`right` are character
/// counts. Parsed from `margin = [left, right]` (len 2) or
/// `margin = [top, right, bottom, left]` (len 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Margin {
    pub(crate) top: usize,
    pub(crate) right: usize,
    pub(crate) bottom: usize,
    pub(crate) left: usize,
}

/// A rendered element; each maps to a theme term chain the resolver cascades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Element {
    Heading(u8),
    Paragraph,
    BlockQuote,
    CodeText,
    CodeContent,
    InlineCode,
    Emphasis,
    Strong,
    Strikethrough,
    Link,
    UnableLinkText,
    UnableLinkDest,
    LinkRef,
    ListMarker,
    Rule,
}

impl Element {
    /// The theme term paths, least to most specific. The resolver folds these
    /// (`all` first) so a specific term overrides the group and `all`.
    fn term_paths(&self) -> Vec<Vec<&'static str>> {
        match self {
            Element::Heading(n) => {
                let lvl = match n {
                    1 => "1",
                    2 => "2",
                    3 => "3",
                    4 => "4",
                    5 => "5",
                    _ => "6",
                };
                vec![vec!["all"], vec!["header"], vec!["header", lvl]]
            }
            Element::Paragraph => vec![vec!["all"]],
            Element::BlockQuote => vec![vec!["all"], vec!["blockquote"]],
            Element::CodeText => vec![vec!["all"], vec!["code"]],
            Element::CodeContent => vec![vec!["code", "content"]],
            Element::InlineCode => vec![vec!["all"], vec!["codespan"]],
            Element::Emphasis => vec![vec!["all"], vec!["italic"]],
            Element::Strong => vec![vec!["all"], vec!["bold"]],
            Element::Strikethrough => vec![vec!["all"], vec!["strikethru"]],
            Element::Link => vec![vec!["all"], vec!["link"]],
            Element::UnableLinkText => vec![
                vec!["all"],
                vec!["unable"],
                vec!["unable", "link"],
                vec!["unable", "link", "text"],
            ],
            Element::UnableLinkDest => vec![
                vec!["all"],
                vec!["unable"],
                vec!["unable", "link"],
                vec!["unable", "link", "dest"],
            ],
            Element::LinkRef => vec![vec!["all"], vec!["reference"]],
            Element::ListMarker => vec![vec!["all"], vec!["list"]],
            Element::Rule => vec![vec!["all"], vec!["rule"]],
        }
    }
}

/// nfo.toml `[render] reference` - when to print the author's `[label]: url`
/// link reference definitions at the bottom (NOT `[^x]` footnotes). `Always`
/// lists all, `Unused` (default) only orphans (defined, never referenced by a
/// link), `Never` none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReferenceMode {
    Always,
    Unused,
    Never,
}

/// A resolved theme: the reference mode plus the palette (dot-walk terms ->
/// `.fg`/`.bg`/`.style`, kept as the parsed TOML table so it round-trips).
#[derive(Clone, Debug)]
pub(crate) struct Theme {
    pub(crate) reference: ReferenceMode,
    pub(crate) palette: toml::Table,
    pub(crate) kind: ComponentKind,
}

impl Theme {
    /// The resolved `Style` for an element (cascade `all` -> group -> specific).
    pub(crate) fn style(&self, el: Element) -> Style {
        let mut s = Style::default();
        for path in el.term_paths() {
            if let Some(node) = node_at(&self.palette, &path) {
                s = s.over(style_of(node));
            }
        }
        s
    }

    /// Whether an element's resolved `.style` includes `markup` (show the source
    /// markdown markup, e.g. the `# ` on a heading).
    pub(crate) fn shows_md(&self, el: Element) -> bool {
        let mut explicit: Option<bool> = None;
        let mut implied = false;
        for path in el.term_paths() {
            let mut md = path.clone();
            md.push("md");
            if let Some(node) = node_at(&self.palette, &md) {
                if let Some(r) = node.get("render").and_then(|v| v.as_bool()) {
                    explicit = Some(r);
                }
                if node.contains_key("fg") || node.contains_key("bg") || has_any_style(node) {
                    implied = true;
                }
            }
        }
        explicit.unwrap_or(implied)
    }

    /// The style for an element's markup glyph, from its `md` sub-terms
    /// (`md.fg`/`md.bg`/`md.style`), cascaded `all.md` -> specific. Overlaid on
    /// the element's own style by the renderer, so `md` only overrides.
    pub(crate) fn md_style(&self, el: Element) -> Style {
        let mut s = Style::default();
        for path in el.term_paths() {
            let mut md = path.clone();
            md.push("md");
            if let Some(node) = node_at(&self.palette, &md) {
                s = s.over(style_of(node));
            }
        }
        s
    }

    /// The style for a named sub-part of an element's markup (e.g. a code
    /// fence's `fence` delimiter or `info` string), from the element's
    /// `md.<sub>` sub-terms, cascaded `all.md.<sub>` -> specific.
    pub(crate) fn md_sub_style(&self, el: Element, sub: &str) -> Style {
        let mut s = Style::default();
        for path in el.term_paths() {
            let mut p = path.clone();
            p.push("md");
            p.push(sub);
            if let Some(node) = node_at(&self.palette, &p) {
                s = s.over(style_of(node));
            }
        }
        s
    }

    /// The resolved `margin` for an element (the most specific term that sets a
    /// `margin`). Absent -> zero.
    pub(crate) fn margin(&self, el: Element) -> Margin {
        let mut out = Margin::default();
        for path in el.term_paths() {
            if let Some(m) = node_at(&self.palette, &path).and_then(read_margin) {
                out = m;
            }
        }
        out
    }

    /// The `[body]` section's margin (document layout): `top`/`bottom` blank
    /// lines, `left`/`right` character margins. Absent -> zero.
    pub(crate) fn body_margin(&self) -> Margin {
        self.palette
            .get("body")
            .and_then(|v| v.as_table())
            .and_then(read_margin)
            .unwrap_or_default()
    }
}

/// The built-in default palette (parsed from the embedded default theme).
pub(crate) fn default_palette() -> toml::Table {
    toml::from_str(DEFAULT_THEME_TOML).expect("embedded default_theme.toml is valid")
}

/// Walk the palette to the table at `path` (e.g. `["header", "1"]`).
fn node_at<'a>(palette: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Table> {
    let mut node = palette.get(path[0])?.as_table()?;
    for seg in &path[1..] {
        node = node.get(*seg)?.as_table()?;
    }
    Some(node)
}

/// The `.fg`/`.bg`/`.style` of one term node -> a render `Style` (ignoring any
/// nested sub-term tables).
fn style_of(node: &toml::Table) -> Style {
    Style {
        fg: read_color(node, "fg"),
        bg: read_color(node, "bg"),
        bold: has_attr(node, "bold"),
        italic: has_attr(node, "italic"),
        underline: has_attr(node, "underline"),
        strikethrough: has_attr(node, "strikethru"),
    }
}

/// Read an `[r, g, b]` array key into a `Color` (values clamped to 0-255).
fn read_color(node: &toml::Table, key: &str) -> Option<Color> {
    let a = node.get(key)?.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let ch = |i: usize| a[i].as_integer().map(|v| v.clamp(0, 255) as u8);
    Some(Color { r: ch(0)?, g: ch(1)?, b: ch(2)? })
}

/// Whether the term node's `.style` list contains `attr`.
fn has_attr(node: &toml::Table, attr: &str) -> bool {
    node.get("style")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().any(|s| s.as_str() == Some(attr)))
        .unwrap_or(false)
}

/// Whether a term node sets a non-empty `style` list.
fn has_any_style(node: &toml::Table) -> bool {
    node.get("style").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false)
}

/// Read a node's `margin`: `[left, right]` (len 2) or `[top, right, bottom,
/// left]` (len 4), non-negative.
fn read_margin(node: &toml::Table) -> Option<Margin> {
    let a = node.get("margin")?.as_array()?;
    let n = |i: usize| a[i].as_integer().filter(|v| *v >= 0).map(|v| v as usize);
    match a.len() {
        2 => Some(Margin { left: n(0)?, right: n(1)?, ..Margin::default() }),
        4 => Some(Margin { top: n(0)?, right: n(1)?, bottom: n(2)?, left: n(3)? }),
        _ => None,
    }
}
