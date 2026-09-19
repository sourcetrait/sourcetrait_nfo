use crate::*;

/// Enhanced markdown display
#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// Markdown file to display
    pub(crate) file: PathBuf,

    /// Configuration profile
    #[arg(short = 'p', long = "profile", default_value_t = Cow::Borrowed("default"))]
    pub(crate) profile: Cow<'static, str>,
}
