pub(crate) mod r {
    pub(crate) mod md {
        pub(crate) use pulldown_cmark::{
            CodeBlockKind, Event, Options, Parser, Tag,
        };
    }
}

pub(crate) mod component;
pub(crate) mod config;
pub(crate) mod doc;
pub(crate) mod error;
pub(crate) mod parse;
pub(crate) mod render;
pub(crate) mod theme;

pub(crate) use std::collections::HashSet;

pub(crate) use snafu::ResultExt;

pub(crate) use crate::doc::{Block, Document, Inline, RefDef};
pub(crate) use crate::error::{ConfigIoSnafu, ConfigParseSnafu};
pub(crate) use crate::parse::parse_markdown;
pub(crate) use crate::theme::{
    default_palette, Element, ReferenceMode, Style, Theme, DEFAULT_THEME_TOML,
};

pub use crate::component::ComponentKind;
pub use crate::config::Config;
pub use crate::error::{LibError, LibResult};
pub use crate::render::render_markdown_ansi;

#[cfg(test)]
mod tests {
    mod config;
}
