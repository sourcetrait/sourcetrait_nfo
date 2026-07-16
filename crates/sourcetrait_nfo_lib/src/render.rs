use crate::*;

/// Render a markdown string to an ANSI-styled terminal string. Wrapping follows
/// `config`'s `[wrap]` band (capped by `width`, the terminal columns); styling
/// follows the loaded theme. The component supplies `width` + the config.
pub fn render_markdown_ansi(
    markdown: &str,
    width: usize,
    config: &Config,
    kind: ComponentKind,
) -> String {
    let theme = Theme {
        reference: config.render.reference,
        palette: config.palette.clone(),
        kind,
    };
    // Wrap band in content columns (after the left margin), capped by terminal.
    let content = width.saturating_sub(theme.body_margin().left.max(0) as usize).max(1);
    let max = config.wrap.max.min(content).max(1);
    let band = Band { min: config.wrap.min.min(max), max };
    render_to_ansi(&parse_markdown(markdown), &theme, band)
}

/// A wrap band in columns: fill until a line exceeds `min` (then wrap on the
/// next token); never exceed `max`. Nesting shrinks both by the prefix width.
#[derive(Clone, Copy)]
pub(crate) struct Band {
    min: usize,
    max: usize,
}

impl Band {
    fn shrink(self, n: usize) -> Band {
        Band { min: self.min.saturating_sub(n).max(1), max: self.max.saturating_sub(n).max(1) }
    }
}

/// Render a parsed `Document` with an explicit theme and wrap band.
pub(crate) fn render_to_ansi(doc: &Document, theme: &Theme, band: Band) -> String {
    let bm = theme.body_margin();
    let mut lines = render_blocks(theme, &doc.blocks, band, Style::default());

    let refs: Vec<&RefDef> = match theme.reference {
        ReferenceMode::Never => Vec::new(),
        ReferenceMode::Always => doc.references.iter().collect(),
        ReferenceMode::Unused => doc.references.iter().filter(|d| !d.used).collect(),
    };
    if !refs.is_empty() {
        lines.push(Vec::new());
        let ref_style = theme.style(Element::LinkRef);
        for d in refs {
            lines.push(vec![Run { text: format!("[{}]: {}", d.label, d.dest), style: ref_style, link: None }]);
        }
    }

    let left = bm.left.max(0) as usize;
    let mut out = String::new();
    for _ in 0..(bm.top.max(0) as usize) {
        out.push('\n');
    }
    for line in &lines {
        emit_line(&mut out, left, line);
    }
    for _ in 0..(bm.bottom.max(0) as usize) {
        out.push('\n');
    }
    out
}

/// A styled text run: text + resolved style + an optional OSC 8 link target.
/// Serialized to ANSI at emit.
struct Run {
    text: String,
    style: Style,
    link: Option<String>,
}

/// A rendered line is a sequence of styled runs. An empty line is a blank row.
type Line = Vec<Run>;

/// An inline flow token fed to the wrapper. Word/Space carry an optional OSC 8
/// link target so an able link stays a live hyperlink through wrapping.
enum Token {
    Word(String, Style, Option<String>),
    Space(Style, Option<String>),
    Hard,
}

/// Render a block sequence to lines, separating blocks with one blank line.
fn render_blocks(theme: &Theme, blocks: &[Block], band: Band, base: Style) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            // One blank row between blocks, reduced by this block's negative top
            // margin (read forward: the block pulls up toward the one before it).
            let top = theme.margin(block_element(block)).top;
            for _ in 0..((1 + top).max(0) as usize) {
                out.push(Vec::new());
            }
        }
        out.extend(block_lines(theme, block, band, base));
    }
    out
}

/// The theme Element a block maps to (for its margin lookup).
fn block_element(block: &Block) -> Element {
    match block {
        Block::Heading { level, .. } => Element::Heading(*level),
        Block::Paragraph(_) => Element::Paragraph,
        Block::Quote(_) => Element::BlockQuote,
        Block::Code { .. } => Element::CodeText,
        Block::List { .. } => Element::ListMarker,
        Block::Rule => Element::Rule,
    }
}

/// Render one block to its lines. All internal indentation (code inset,
/// blockquote bar, list marker) is baked into the runs; the caller adds the
/// outer body margin.
fn block_lines(theme: &Theme, block: &Block, band: Band, base: Style) -> Vec<Line> {
    match block {
        Block::Heading { level, inlines } => {
            let mut toks: Vec<Token> = Vec::new();
            let hstyle = base.over(theme.style(Element::Heading(*level)));
            if theme.shows_md(Element::Heading(*level)) {
                let mstyle = hstyle.over(theme.md_style(Element::Heading(*level)));
                toks.push(Token::Word("#".repeat(*level as usize), mstyle, None));
                toks.push(Token::Space(mstyle, None));
            }
            inline_tokens(theme, inlines, hstyle, None, &mut toks);
            wrap(toks, band)
        }
        Block::Paragraph(inlines) => {
            let mut toks: Vec<Token> = Vec::new();
            inline_tokens(theme, inlines, base.over(theme.style(Element::Paragraph)), None, &mut toks);
            wrap(toks, band)
        }
        Block::Code { text, lang } => {
            let cstyle = base.over(theme.style(Element::CodeText));
            let markup = cstyle.over(theme.md_style(Element::CodeText));
            let block_left = theme.margin(Element::CodeText).left;
            let fence_pad = " ".repeat(block_left.max(0) as usize);
            let content_pad =
                " ".repeat((block_left + theme.margin(Element::CodeContent).left).max(0) as usize);
            let show = theme.shows_md(Element::CodeText);
            let fstyle = markup.over(theme.md_sub_style(Element::CodeText, "fence"));
            let mut out: Vec<Line> = Vec::new();
            if show {
                let mut line = vec![Run { text: format!("{fence_pad}```"), style: fstyle, link: None }];
                if let Some(lang) = lang.as_deref().filter(|l| !l.is_empty()) {
                    let istyle = markup.over(theme.md_sub_style(Element::CodeText, "info"));
                    line.push(Run { text: lang.to_string(), style: istyle, link: None });
                }
                out.push(line);
            }
            for raw in text.trim_end_matches('\n').split('\n') {
                out.push(vec![Run { text: format!("{content_pad}{raw}"), style: cstyle, link: None }]);
            }
            if show {
                out.push(vec![Run { text: format!("{fence_pad}```"), style: fstyle, link: None }]);
            }
            out
        }
        Block::Quote(inner) => {
            let qstyle = base.over(theme.style(Element::BlockQuote));
            // The blockquote's markdown markup is `> `; md-gated like any markup.
            let show = theme.shows_md(Element::BlockQuote);
            let inner_band = band.shrink(if show { 2 } else { 0 });
            let bstyle = qstyle.over(theme.md_style(Element::BlockQuote));
            render_blocks(theme, inner, inner_band, qstyle)
                .into_iter()
                .map(|mut ln| {
                    if show {
                        let mut line = vec![Run { text: "> ".to_string(), style: bstyle, link: None }];
                        line.append(&mut ln);
                        line
                    } else {
                        ln
                    }
                })
                .collect()
        }
        Block::List { start, items } => {
            let mut out: Vec<Line> = Vec::new();
            let mstyle = theme.style(Element::ListMarker);
            for (idx, item) in items.iter().enumerate() {
                let marker = match start {
                    Some(n) => format!("{}. ", n + idx as u64),
                    None => "- ".to_string(),
                };
                let mw = marker.chars().count();
                let item_band = band.shrink(mw);
                for (li, mut ln) in render_blocks(theme, item, item_band, base).into_iter().enumerate() {
                    let prefix = if li == 0 {
                        Run { text: marker.clone(), style: mstyle, link: None }
                    } else {
                        Run { text: " ".repeat(mw), style: Style::default(), link: None }
                    };
                    let mut line = vec![prefix];
                    line.append(&mut ln);
                    out.push(line);
                }
            }
            out
        }
        Block::Rule => vec![vec![Run {
            text: "\u{2500}".repeat(band.max),
            style: theme.style(Element::Rule),
            link: None,
        }]],
    }
}

/// Flatten an inline sequence into flow tokens under a base style. `link` is the
/// OSC 8 target in force (Some inside an able link), carried onto every token so
/// the hyperlink survives wrapping.
fn inline_tokens(theme: &Theme, inlines: &[Inline], base: Style, link: Option<&str>, out: &mut Vec<Token>) {
    for inl in inlines {
        match inl {
            Inline::Text(t) => push_text(t, base, link, out),
            Inline::Code(t) => {
                let content = base.over(theme.style(Element::InlineCode));
                if theme.shows_md(Element::InlineCode) {
                    let mstyle = content.over(theme.md_style(Element::InlineCode));
                    out.push(Token::Word("`".to_string(), mstyle, link.map(str::to_string)));
                    out.push(Token::Word(t.clone(), content, link.map(str::to_string)));
                    out.push(Token::Word("`".to_string(), mstyle, link.map(str::to_string)));
                } else {
                    out.push(Token::Word(t.clone(), content, link.map(str::to_string)));
                }
            }
            Inline::Emphasis(v) => marker_wrap(theme, Element::Emphasis, "*", v, base, link, out),
            Inline::Strong(v) => marker_wrap(theme, Element::Strong, "**", v, base, link, out),
            Inline::Strikethrough(v) => marker_wrap(theme, Element::Strikethrough, "~~", v, base, link, out),
            Inline::Link { text, dest } => {
                if theme.kind.marks_unable(dest) {
                    // Unable link (cli + local file): no OSC 8. The dest is the
                    // link's markdown, so surfacing it inline is md-gated.
                    inline_tokens(theme, text, base.over(theme.style(Element::UnableLinkText)), link, out);
                    if theme.shows_md(Element::UnableLinkDest) {
                        let m = theme.margin(Element::UnableLinkDest);
                        if m.left > 0 {
                            out.push(Token::Space(base, None));
                        }
                        let pad = if m.left > 1 { " ".repeat((m.left - 1) as usize) } else { String::new() };
                        let dstyle = base.over(theme.style(Element::UnableLinkDest));
                        out.push(Token::Word(format!("{pad}({dest}){}", " ".repeat(m.right.max(0) as usize)), dstyle, None));
                    }
                } else {
                    // Able link: its text becomes a live OSC 8 hyperlink to dest.
                    inline_tokens(theme, text, base.over(theme.style(Element::Link)), Some(dest), out);
                }
            }
            Inline::SoftBreak => out.push(Token::Space(base, link.map(str::to_string))),
            Inline::HardBreak => out.push(Token::Hard),
        }
    }
}

/// Split text into word / space tokens, collapsing runs of whitespace. The
/// in-force `link` rides along on each token.
fn push_text(t: &str, style: Style, link: Option<&str>, out: &mut Vec<Token>) {
    let mut word = String::new();
    for ch in t.chars() {
        if ch.is_whitespace() {
            if !word.is_empty() {
                out.push(Token::Word(std::mem::take(&mut word), style, link.map(str::to_string)));
            }
            out.push(Token::Space(style, link.map(str::to_string)));
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        out.push(Token::Word(word, style, link.map(str::to_string)));
    }
}

/// Render a nested inline element (emphasis / strong / strikethrough), wrapping
/// its content with the markdown `marker` when the element's `md.render` is on.
/// Any in-force `link` is carried through (markers included).
fn marker_wrap(theme: &Theme, el: Element, marker: &str, v: &[Inline], base: Style, link: Option<&str>, out: &mut Vec<Token>) {
    let content = base.over(theme.style(el));
    if theme.shows_md(el) {
        let mstyle = content.over(theme.md_style(el));
        out.push(Token::Word(marker.to_string(), mstyle, link.map(str::to_string)));
        inline_tokens(theme, v, content, link, out);
        out.push(Token::Word(marker.to_string(), mstyle, link.map(str::to_string)));
    } else {
        inline_tokens(theme, v, content, link, out);
    }
}

/// Word-wrap flow tokens into lines per the `band`: a line fills until it
/// exceeds `band.min`, then the next token wraps; a token is never placed past
/// `band.max` (a word wider than `max` takes its own line and overflows). Widths
/// are char-counts; grapheme/display-width is a later slice.
fn wrap(tokens: Vec<Token>, band: Band) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut cur: Line = Vec::new();
    let mut cur_w = 0usize;
    let mut pending_space: Option<(Style, Option<String>)> = None;

    for tok in tokens {
        match tok {
            Token::Hard => {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0;
                pending_space = None;
            }
            Token::Space(s, link) => {
                if cur_w > 0 {
                    pending_space = Some((s, link));
                }
            }
            Token::Word(text, style, link) => {
                let w = text.chars().count();
                let extra = usize::from(pending_space.is_some());
                let would_be = cur_w + extra + w;
                // Wrap once the line has exceeded `min` (on this next token), or
                // whenever the token would cross `max` (the hard ceiling).
                if cur_w > 0 && (cur_w > band.min || would_be > band.max) {
                    lines.push(std::mem::take(&mut cur));
                    cur_w = w;
                    pending_space = None;
                    cur.push(Run { text, style, link });
                } else {
                    if let Some((s, slink)) = pending_space.take() {
                        cur.push(Run { text: " ".to_string(), style: s, link: slink });
                        cur_w += 1;
                    }
                    cur_w += w;
                    cur.push(Run { text, style, link });
                }
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Emit one line: the outer margin, then each run's ANSI. Consecutive runs that
/// share an OSC 8 link target are wrapped in one hyperlink, re-opened + closed
/// PER LINE (a single escape spanning a newline breaks some terminals). An empty
/// line is a bare blank row (no trailing margin whitespace).
fn emit_line(out: &mut String, margin: usize, line: &[Run]) {
    if line.is_empty() {
        out.push('\n');
        return;
    }
    for _ in 0..margin {
        out.push(' ');
    }
    let mut open: Option<&str> = None;
    for run in line {
        let target = run.link.as_deref();
        if target != open {
            if open.is_some() {
                out.push_str("\u{1b}]8;;\u{1b}\\");
            }
            if let Some(uri) = target {
                out.push_str("\u{1b}]8;;");
                out.push_str(uri);
                out.push_str("\u{1b}\\");
            }
            open = target;
        }
        out.push_str(&ansi(run.style, &run.text));
    }
    if open.is_some() {
        out.push_str("\u{1b}]8;;\u{1b}\\");
    }
    out.push('\n');
}

/// Serialize a styled run to an ANSI SGR sequence (truecolor), or the bare text
/// when the style is plain.
fn ansi(style: Style, text: &str) -> String {
    if style.is_plain() {
        return text.to_string();
    }
    let mut codes: Vec<String> = Vec::new();
    if style.bold {
        codes.push("1".to_string());
    }
    if style.italic {
        codes.push("3".to_string());
    }
    if style.underline {
        codes.push("4".to_string());
        // Explicit underline color (= fg) so a terminal does not substitute its
        // own for an OSC 8 link's underline.
        if let Some(c) = style.fg {
            codes.push(format!("58;2;{};{};{}", c.r, c.g, c.b));
        }
    }
    if style.strikethrough {
        codes.push("9".to_string());
    }
    if let Some(c) = style.fg {
        codes.push(format!("38;2;{};{};{}", c.r, c.g, c.b));
    }
    if let Some(c) = style.bg {
        codes.push(format!("48;2;{};{};{}", c.r, c.g, c.b));
    }
    format!("\u{1b}[{}m{}\u{1b}[0m", codes.join(";"), text)
}
