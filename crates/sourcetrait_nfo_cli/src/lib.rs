pub(crate) mod cli;
pub(crate) mod run;

pub(crate) use std::{
    io::{
        self,
        Write,
    },
    process::ExitCode,
};

pub(crate) use clap::Parser;
pub(crate) use sourcetrait_nfo_lib as lib;

pub(crate) use crate::cli::Args;

pub use crate::run::run;
