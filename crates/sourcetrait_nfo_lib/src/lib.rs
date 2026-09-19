pub(crate) mod component;
pub(crate) mod config;
pub(crate) mod doc;
pub(crate) mod error;
pub(crate) mod parse;
pub(crate) mod render;
pub(crate) mod theme;

pub(crate) use crate::{
    doc::{Block, Document, Inline, RefDef},
    error::{ConfigIoSnafu, ConfigParseSnafu},
    parse::parse_markdown,
    theme::{
        default_palette, Element, Style, Theme, DEFAULT_THEME_TOML,
    },
};

pub use crate::{
    config::{Config, RenderConfig, WrapConfig},
    error::{LibError, LibResult},
    component::NfoSuite,
    theme::ReferenceMode,
    render::render_markdown_ansi,
};

pub(crate) use std::{
    collections::HashSet
};

pub(crate) use snafu::ResultExt;

pub(crate) mod r {
    pub(crate) mod md {
        pub(crate) use pulldown_cmark::{
            CodeBlockKind, Event, Options, Parser, Tag,
        };
    }
}

