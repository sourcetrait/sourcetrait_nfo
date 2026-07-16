use crate::*;

/// Parse a markdown string into the backend-agnostic `Document`.
///
/// Uses pulldown-cmark with a slice-1 option set (strikethrough + task lists).
/// The author's `[label]: dest` reference definitions are read from the parser
/// and each flagged used/unused (whether a reference link resolved it); the
/// renderer chooses which to show. The result is owned (`'static`).
pub(crate) fn parse_markdown(md: &str) -> Document {
    let mut opts = r::md::Options::empty();
    opts.insert(r::md::Options::ENABLE_STRIKETHROUGH);
    opts.insert(r::md::Options::ENABLE_TASKLISTS);

    let parser = r::md::Parser::new_ext(md, opts);

    // The author's reference definitions, parsed in the first pass. Collect and
    // sort (RefDefs is a hash map - order is otherwise nondeterministic) before
    // the event loop consumes the parser.
    let mut defs: Vec<(String, String)> = parser
        .reference_definitions()
        .iter()
        .map(|(label, def)| (label.to_string(), def.dest.to_string()))
        .collect();
    defs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut b = Builder::new();
    for ev in parser {
        b.event(ev);
    }
    b.finish(defs)
}

/// A partially-built container on the builder stack. Block containers
/// (Root/Quote/Item) carry a `pending` inline buffer so a tight list's bare
/// inlines auto-wrap into a paragraph; inline containers carry their spans.
enum Frame {
    Root { blocks: Vec<Block>, pending: Vec<Inline> },
    Quote { blocks: Vec<Block>, pending: Vec<Inline> },
    Item { blocks: Vec<Block>, pending: Vec<Inline> },
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Para(Vec<Inline>),
    Heading { level: u8, inlines: Vec<Inline> },
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Link { text: Vec<Inline>, dest: String },
    Code { text: String, lang: Option<String> },
}

struct Builder {
    stack: Vec<Frame>,
    /// Labels of reference definitions actually resolved by a link.
    used: HashSet<String>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            stack: vec![Frame::Root { blocks: Vec::new(), pending: Vec::new() }],
            used: HashSet::new(),
        }
    }

    fn top(&mut self) -> &mut Frame {
        self.stack.last_mut().expect("root frame is never popped before finish")
    }

    /// Push an inline into the current sink: an inline frame's span vector, or a
    /// block frame's pending auto-paragraph buffer.
    fn push_inline(&mut self, inline: Inline) {
        match self.top() {
            Frame::Para(v)
            | Frame::Heading { inlines: v, .. }
            | Frame::Emph(v)
            | Frame::Strong(v)
            | Frame::Strike(v) => v.push(inline),
            Frame::Link { text, .. } => text.push(inline),
            Frame::Root { pending, .. }
            | Frame::Quote { pending, .. }
            | Frame::Item { pending, .. } => pending.push(inline),
            Frame::List { .. } | Frame::Code { .. } => {}
        }
    }

    /// Flush the top block frame's pending inlines into a paragraph block.
    fn flush_pending(&mut self) {
        match self.top() {
            Frame::Root { blocks, pending }
            | Frame::Quote { blocks, pending }
            | Frame::Item { blocks, pending }
                if !pending.is_empty() =>
            {
                let inlines = std::mem::take(pending);
                blocks.push(Block::Paragraph(inlines));
            }
            _ => {}
        }
    }

    fn push_block(&mut self, block: Block) {
        match self.top() {
            Frame::Root { blocks, .. }
            | Frame::Quote { blocks, .. }
            | Frame::Item { blocks, .. } => blocks.push(block),
            _ => {}
        }
    }

    fn event(&mut self, ev: r::md::Event) {
        match ev {
            r::md::Event::Start(tag) => self.start(tag),
            r::md::Event::End(_) => self.end(),
            r::md::Event::Text(s) => match self.top() {
                Frame::Code { text, .. } => text.push_str(&s),
                _ => self.push_inline(Inline::Text(s.to_string())),
            },
            r::md::Event::Code(s) => self.push_inline(Inline::Code(s.to_string())),
            r::md::Event::Html(s) | r::md::Event::InlineHtml(s) => match self.top() {
                Frame::Code { text, .. } => text.push_str(&s),
                _ => self.push_inline(Inline::Text(s.to_string())),
            },
            r::md::Event::SoftBreak => self.push_inline(Inline::SoftBreak),
            r::md::Event::HardBreak => self.push_inline(Inline::HardBreak),
            r::md::Event::Rule => {
                self.flush_pending();
                self.push_block(Block::Rule);
            }
            r::md::Event::TaskListMarker(done) => {
                self.push_inline(Inline::Text(if done { "[x] ".to_string() } else { "[ ] ".to_string() }));
            }
            r::md::Event::InlineMath(s) => self.push_inline(Inline::Code(s.to_string())),
            r::md::Event::DisplayMath(s) => {
                self.flush_pending();
                self.push_block(Block::Code { text: s.to_string(), lang: None });
            }
            r::md::Event::FootnoteReference(s) => {
                self.push_inline(Inline::Text(format!("[^{s}]")));
            }
        }
    }

    fn start(&mut self, tag: r::md::Tag) {
        match tag {
            r::md::Tag::Paragraph => {
                self.flush_pending();
                self.stack.push(Frame::Para(Vec::new()));
            }
            r::md::Tag::Heading { level, .. } => {
                self.flush_pending();
                self.stack.push(Frame::Heading { level: level as u8, inlines: Vec::new() });
            }
            r::md::Tag::BlockQuote(_) => {
                self.flush_pending();
                self.stack.push(Frame::Quote { blocks: Vec::new(), pending: Vec::new() });
            }
            r::md::Tag::CodeBlock(kind) => {
                self.flush_pending();
                let lang = match kind {
                    r::md::CodeBlockKind::Fenced(l) if !l.is_empty() => Some(l.to_string()),
                    _ => None,
                };
                self.stack.push(Frame::Code { text: String::new(), lang });
            }
            r::md::Tag::List(start) => {
                self.flush_pending();
                self.stack.push(Frame::List { start, items: Vec::new() });
            }
            r::md::Tag::Item => self.stack.push(Frame::Item { blocks: Vec::new(), pending: Vec::new() }),
            r::md::Tag::Emphasis => self.stack.push(Frame::Emph(Vec::new())),
            r::md::Tag::Strong => self.stack.push(Frame::Strong(Vec::new())),
            r::md::Tag::Strikethrough => self.stack.push(Frame::Strike(Vec::new())),
            r::md::Tag::Link { id, dest_url, .. } | r::md::Tag::Image { id, dest_url, .. } => {
                // A non-empty id means a reference link resolved that definition.
                if !id.is_empty() {
                    self.used.insert(id.to_string());
                }
                self.stack.push(Frame::Link { text: Vec::new(), dest: dest_url.to_string() });
            }
            // Not yet modeled (html blocks, definition lists, etc.): collect the
            // children as a passthrough paragraph so their text still renders.
            _ => self.stack.push(Frame::Para(Vec::new())),
        }
    }

    fn end(&mut self) {
        // Finalize on the popped frame, not the TagEnd: Start pushed a matching
        // frame and the parser guarantees balanced start/end events.
        let frame = self.stack.pop().expect("balanced start/end");
        match frame {
            Frame::Para(inlines) => self.push_block(Block::Paragraph(inlines)),
            Frame::Heading { level, inlines } => self.push_block(Block::Heading { level, inlines }),
            Frame::Quote { mut blocks, pending } => {
                if !pending.is_empty() {
                    blocks.push(Block::Paragraph(pending));
                }
                self.push_block(Block::Quote(blocks));
            }
            Frame::List { start, items } => self.push_block(Block::List { start, items }),
            Frame::Item { mut blocks, pending } => {
                if !pending.is_empty() {
                    blocks.push(Block::Paragraph(pending));
                }
                if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                    items.push(blocks);
                }
            }
            Frame::Code { text, lang } => self.push_block(Block::Code { text, lang }),
            Frame::Emph(inlines) => self.push_inline(Inline::Emphasis(inlines)),
            Frame::Strong(inlines) => self.push_inline(Inline::Strong(inlines)),
            Frame::Strike(inlines) => self.push_inline(Inline::Strikethrough(inlines)),
            Frame::Link { text, dest } => self.push_inline(Inline::Link { text, dest }),
            Frame::Root { .. } => {}
        }
    }

    fn finish(mut self, defs: Vec<(String, String)>) -> Document {
        self.flush_pending();
        let blocks = match self.stack.pop() {
            Some(Frame::Root { blocks, .. }) => blocks,
            _ => Vec::new(),
        };
        let references = defs
            .into_iter()
            .map(|(label, dest)| {
                let used = self.used.contains(&label);
                RefDef { label, dest, used }
            })
            .collect();
        Document { blocks, references }
    }
}
