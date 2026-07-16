#[allow(unused_imports)]
use crate::*;

/// The parsed, styled-agnostic document model - the backend-agnostic IR that
/// every UI renders. `lib` owns parsing into this (parse) and rendering out of
/// it (render); the UI components only display it. The model is SEMANTIC: it
/// carries structure, not colors - the theme resolves style at render time.
#[derive(Clone, Debug)]
pub(crate) struct Document {
    pub(crate) blocks: Vec<Block>,
    /// The author's link reference definitions (`[label]: dest`), each flagged
    /// used/unused. The renderer shows all / only-unused / none per the
    /// reference mode. Sorted by label for stable output.
    pub(crate) references: Vec<RefDef>,
}

/// A link reference definition (`[label]: dest`) and whether a link resolved it.
#[derive(Clone, Debug)]
pub(crate) struct RefDef {
    pub(crate) label: String,
    pub(crate) dest: String,
    pub(crate) used: bool,
}

/// A block-level element. Slice-1 set; tables, html blocks, footnote
/// definitions, and definition lists arrive with their own slices.
#[derive(Clone, Debug)]
pub(crate) enum Block {
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph(Vec<Inline>),
    Quote(Vec<Block>),
    /// Fenced or indented code, rendered verbatim. `lang` is the fence info
    /// string (None for indented/bare). Syntax highlighting arrives later.
    Code { text: String, lang: Option<String> },
    /// `start` is `Some(n)` for an ordered list beginning at `n`, `None` for a
    /// bullet list. Each item is its own block sequence.
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Rule,
}

/// An inline element within a block.
#[derive(Clone, Debug)]
pub(crate) enum Inline {
    Text(String),
    /// An inline code span.
    Code(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    /// A hyperlink: its inline `text` and its resolved `dest`. A renderer that
    /// cannot make it a live hyperlink (an Unable link - e.g. a local file link
    /// in the cli) surfaces the dest inline; otherwise the dest drives an OSC 8
    /// hyperlink.
    Link { text: Vec<Inline>, dest: String },
    SoftBreak,
    HardBreak,
}
