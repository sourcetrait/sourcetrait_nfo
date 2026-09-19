pub(crate) mod cli;
pub(crate) mod run;

pub(crate) use crate::{
    cli::Args
};

pub(crate) use std::{
    borrow::Cow,
    io::{
        self,
        Write,
    },
    path::PathBuf,
    process::ExitCode,
};

pub(crate) use clap::Parser;
pub(crate) use sourcetrait_nfo_lib as lib;


pub use crate::run::run;
