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
    let content = width.saturating_sub(theme.body_margin().left).max(1);
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

/// Render a parsed `Document` with an explicit theme and width.
pub(crate) fn render_to_ansi(doc: &Document, theme: &Theme, band: Band) -> String {
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
            lines.push(vec![Run {
                text: format!("[{}]: {}", d.label, d.dest),
                style: ref_style,
            }]);
        }
    }

    let bm = theme.body_margin();
    let mut out = String::new();
    for _ in 0..bm.top {
        out.push('\n');
    }
    for line in &lines {
        emit_line(&mut out, bm.left, line);
    }
    for _ in 0..bm.bottom {
        out.push('\n');
    }
    out
}

/// A styled text run: text plus its resolved style. Serialized to ANSI at emit.
struct Run {
    text: String,
    style: Style,
}

/// A rendered line is a sequence of styled runs. An empty line is a blank row.
type Line = Vec<Run>;

/// An inline flow token fed to the wrapper.
enum Token {
    Word(String, Style),
    Space(Style),
    Hard,
}

/// Render a block sequence to lines, separating blocks with one blank line.
fn render_blocks(theme: &Theme, blocks: &[Block], band: Band, base: Style) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            out.push(Vec::new());
        }
        out.extend(block_lines(theme, block, band, base));
    }
    out
}

/// Render one block to its lines. All internal indentation (code inset,
/// blockquote bar, list marker) is baked into the runs; the caller only adds
/// the outer margin.
fn block_lines(theme: &Theme, block: &Block, band: Band, base: Style) -> Vec<Line> {
    match block {
        Block::Heading { level, inlines } => {
            let mut toks: Vec<Token> = Vec::new();
            let hstyle = base.over(theme.style(Element::Heading(*level)));
            if theme.shows_md(Element::Heading(*level)) {
                let mstyle = hstyle.over(theme.md_style(Element::Heading(*level)));
                toks.push(Token::Word("#".repeat(*level as usize), mstyle));
                toks.push(Token::Space(mstyle));
            }
            inline_tokens(theme, inlines, hstyle, &mut toks);
            wrap(toks, band)
        }
        Block::Paragraph(inlines) => {
            let mut toks: Vec<Token> = Vec::new();
            inline_tokens(theme, inlines, base.over(theme.style(Element::Paragraph)), &mut toks);
            wrap(toks, band)
        }
        Block::Code { text, lang } => {
            let cstyle = base.over(theme.style(Element::CodeText));
            let markup = cstyle.over(theme.md_style(Element::CodeText));
            let block_left = theme.margin(Element::CodeText).left;
            let fence_pad = " ".repeat(block_left);
            let content_pad = " ".repeat(block_left + theme.margin(Element::CodeContent).left);
            let show = theme.shows_md(Element::CodeText);
            let fstyle = markup.over(theme.md_sub_style(Element::CodeText, "fence"));
            let mut out: Vec<Line> = Vec::new();
            if show {
                let mut line = vec![Run { text: format!("{fence_pad}```"), style: fstyle }];
                if let Some(lang) = lang.as_deref().filter(|l| !l.is_empty()) {
                    let istyle = markup.over(theme.md_sub_style(Element::CodeText, "info"));
                    line.push(Run { text: lang.to_string(), style: istyle });
                }
                out.push(line);
            }
            for raw in text.trim_end_matches('\n').split('\n') {
                out.push(vec![Run { text: format!("{content_pad}{raw}"), style: cstyle }]);
            }
            if show {
                out.push(vec![Run { text: format!("{fence_pad}```"), style: fstyle }]);
            }
            out
        }
        Block::Quote(inner) => {
            let qstyle = base.over(theme.style(Element::BlockQuote));
            let inner_band = band.shrink(2);
            let bar = theme.style(Element::BlockQuote);
            render_blocks(theme, inner, inner_band, qstyle)
                .into_iter()
                .map(|mut ln| {
                    let mut line = vec![Run { text: "\u{2502} ".to_string(), style: bar }];
                    line.append(&mut ln);
                    line
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
                        Run { text: marker.clone(), style: mstyle }
                    } else {
                        Run { text: " ".repeat(mw), style: Style::default() }
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
        }]],
    }
}

/// Flatten an inline sequence into flow tokens under a base style; nested
/// emphasis overlays its style, links resolve to the link style.
fn inline_tokens(theme: &Theme, inlines: &[Inline], base: Style, out: &mut Vec<Token>) {
    for inl in inlines {
        match inl {
            Inline::Text(t) => push_text(t, base, out),
            Inline::Code(t) => {
                let content = base.over(theme.style(Element::InlineCode));
                if theme.shows_md(Element::InlineCode) {
                    let mstyle = content.over(theme.md_style(Element::InlineCode));
                    out.push(Token::Word("`".to_string(), mstyle));
                    out.push(Token::Word(t.clone(), content));
                    out.push(Token::Word("`".to_string(), mstyle));
                } else {
                    out.push(Token::Word(t.clone(), content));
                }
            }
            Inline::Emphasis(v) => marker_wrap(theme, Element::Emphasis, "*", v, base, out),
            Inline::Strong(v) => marker_wrap(theme, Element::Strong, "**", v, base, out),
            Inline::Strikethrough(v) => marker_wrap(theme, Element::Strikethrough, "~~", v, base, out),
            Inline::Link { text, dest } => {
                if theme.kind.marks_unable(dest) {
                    // Unable link (e.g. cli + local file): no OSC 8. The dest is
                    // the link's markdown, so surfacing it inline is md-gated.
                    inline_tokens(theme, text, base.over(theme.style(Element::UnableLinkText)), out);
                    if theme.shows_md(Element::UnableLinkDest) {
                        let m = theme.margin(Element::UnableLinkDest);
                        if m.left > 0 {
                            out.push(Token::Space(base));
                        }
                        let pad = if m.left > 1 { " ".repeat(m.left - 1) } else { String::new() };
                        let dstyle = base.over(theme.style(Element::UnableLinkDest));
                        out.push(Token::Word(format!("{pad}({dest}){}", " ".repeat(m.right)), dstyle));
                    }
                } else {
                    inline_tokens(theme, text, base.over(theme.style(Element::Link)), out);
                }
            }
            Inline::SoftBreak => out.push(Token::Space(base)),
            Inline::HardBreak => out.push(Token::Hard),
        }
    }
}

/// Render a nested inline element (emphasis / strong / strikethrough), wrapping
/// its content with the markdown `marker` when the element's `md.render` is on.
/// The marker takes the element's style with its `md` overrides on top.
fn marker_wrap(theme: &Theme, el: Element, marker: &str, v: &[Inline], base: Style, out: &mut Vec<Token>) {
    let content = base.over(theme.style(el));
    if theme.shows_md(el) {
        let mstyle = content.over(theme.md_style(el));
        out.push(Token::Word(marker.to_string(), mstyle));
        inline_tokens(theme, v, content, out);
        out.push(Token::Word(marker.to_string(), mstyle));
    } else {
        inline_tokens(theme, v, content, out);
    }
}

/// Split text into word / space tokens, collapsing runs of whitespace.
fn push_text(t: &str, style: Style, out: &mut Vec<Token>) {
    let mut word = String::new();
    for ch in t.chars() {
        if ch.is_whitespace() {
            if !word.is_empty() {
                out.push(Token::Word(std::mem::take(&mut word), style));
            }
            out.push(Token::Space(style));
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        out.push(Token::Word(word, style));
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
    let mut pending_space: Option<Style> = None;

    for tok in tokens {
        match tok {
            Token::Hard => {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0;
                pending_space = None;
            }
            Token::Space(s) => {
                if cur_w > 0 {
                    pending_space = Some(s);
                }
            }
            Token::Word(text, style) => {
                let w = text.chars().count();
                let extra = usize::from(pending_space.is_some());
                let would_be = cur_w + extra + w;
                // Wrap once the line has exceeded `min` (on this next token), or
                // whenever the token would cross `max` (the hard ceiling).
                if cur_w > 0 && (cur_w > band.min || would_be > band.max) {
                    lines.push(std::mem::take(&mut cur));
                    cur_w = w;
                    pending_space = None;
                    cur.push(Run { text, style });
                } else {
                    if let Some(s) = pending_space.take() {
                        cur.push(Run { text: " ".to_string(), style: s });
                        cur_w += 1;
                    }
                    cur_w += w;
                    cur.push(Run { text, style });
                }
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Emit one line: the outer margin, then each run's ANSI. An empty line is a
/// bare blank row (no trailing margin whitespace).
fn emit_line(out: &mut String, margin: usize, line: &[Run]) {
    if line.is_empty() {
        out.push('\n');
        return;
    }
    for _ in 0..margin {
        out.push(' ');
    }
    for run in line {
        out.push_str(&ansi(run.style, &run.text));
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
