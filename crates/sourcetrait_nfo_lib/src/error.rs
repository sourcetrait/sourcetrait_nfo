#[allow(unused_imports)]
use crate::*;

/// The crate result alias.
pub type LibResult<T> = Result<T, LibError>;

/// The crate error. Module-level errors wrap in here as the surface grows.
#[derive(Debug, snafu::Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum LibError {
    #[snafu(display("invalid config: {source}"))]
    ConfigParse { source: toml::de::Error },

    #[snafu(display("config i/o at {path:?}: {source}"))]
    ConfigIo {
        source: std::io::Error,
        path: std::path::PathBuf,
    },
}
