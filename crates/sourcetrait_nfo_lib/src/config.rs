use crate::*;

/// The embedded default config - the single source of truth for the config
/// defaults, and the `-c defaults` template for `profile/defaults/nfo.toml`.
const DEFAULT_TOML: &str = include_str!("default.toml");

/// The nfo configuration: the selected theme name, the `[render]` toggles, and
/// the resolved palette (loaded from `theme/<theme>.toml`).
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_theme_name")]
    pub(crate) theme: String,
    pub(crate) render: RenderConfig,
    pub(crate) wrap: WrapConfig,
    #[serde(skip, default = "default_palette")]
    pub(crate) palette: toml::Table,
}

fn default_theme_name() -> String {
    "defaults".to_string()
}

/// The `[render]` section - the config contract (only these options exist).
#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct RenderConfig {
    pub(crate) reference: ReferenceMode,
}

/// The `[wrap]` section - the wrap band in columns. Wrap once a line exceeds
/// `min` (on the next token); never exceed `max` (the hard ceiling).
#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct WrapConfig {
    pub(crate) min: usize,
    pub(crate) max: usize,
}

impl Config {
    /// The hard-coded defaults (embedded default config + default theme).
    pub fn defaults() -> Config {
        toml::from_str(DEFAULT_TOML).expect("embedded default.toml is valid")
    }

    /// Parse a user config, merged over the embedded defaults (user wins). The
    /// palette stays the default; `load` resolves the selected theme.
    pub fn from_toml(user: &str) -> LibResult<Config> {
        let mut merged: toml::Value =
            toml::from_str(DEFAULT_TOML).expect("embedded default.toml is valid");
        let over: toml::Value = toml::from_str(user).context(ConfigParseSnafu)?;
        merge_value(&mut merged, over);
        merged.try_into().context(ConfigParseSnafu)
    }

    /// Load the config for a profile (`-c <snake>`) or the default set (`None`)
    /// from `$XDG_CONFIG_HOME/sourcetrait/nfo`, then resolve its theme palette.
    pub fn load(profile: Option<&str>) -> LibResult<Config> {
        Config::load_from(&config_root(), profile)
    }

    /// `load`, rooted at an explicit config dir (the seam behind `load`).
    pub(crate) fn load_from(root: &std::path::Path, profile: Option<&str>) -> LibResult<Config> {
        let dir = match profile {
            Some(p) => root.join("profile").join(p),
            None => root.to_path_buf(),
        };
        let mut cfg = match std::fs::read_to_string(dir.join("nfo.toml")) {
            Ok(user) => Config::from_toml(&user)?,
            Err(_) => Config::defaults(),
        };
        // Resolve the selected theme's palette from the config root's theme dir;
        // missing/unreadable/invalid keeps the built-in default palette.
        let theme_path = root.join("theme").join(format!("{}.toml", cfg.theme));
        if let Ok(text) = std::fs::read_to_string(&theme_path) {
            if let Ok(table) = toml::from_str::<toml::Table>(&text) {
                cfg.palette = table;
            }
        }
        Ok(cfg)
    }

    /// Regenerate the built-in defaults - `profile/defaults/nfo.toml` (config)
    /// and `theme/defaults.toml` (theme) - verbatim from the embedded templates;
    /// always overwritten. Returns the paths written.
    pub fn write_defaults() -> LibResult<Vec<std::path::PathBuf>> {
        Config::write_defaults_to(&config_root())
    }

    /// `write_defaults`, rooted at an explicit config dir (the seam).
    pub(crate) fn write_defaults_to(root: &std::path::Path) -> LibResult<Vec<std::path::PathBuf>> {
        let pdir = root.join("profile").join("defaults");
        std::fs::create_dir_all(&pdir).context(ConfigIoSnafu { path: pdir.clone() })?;
        let cfg_path = pdir.join("nfo.toml");
        std::fs::write(&cfg_path, DEFAULT_TOML).context(ConfigIoSnafu { path: cfg_path.clone() })?;

        let tdir = root.join("theme");
        std::fs::create_dir_all(&tdir).context(ConfigIoSnafu { path: tdir.clone() })?;
        let theme_path = tdir.join("defaults.toml");
        std::fs::write(&theme_path, DEFAULT_THEME_TOML)
            .context(ConfigIoSnafu { path: theme_path.clone() })?;

        Ok(vec![cfg_path, theme_path])
    }
}

/// `$XDG_CONFIG_HOME/sourcetrait/nfo` (XDG fallback `~/.config`).
fn config_root() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.config")
    });
    std::path::PathBuf::from(base).join("sourcetrait").join("nfo")
}

/// Deep-merge `over` into `base` (over wins): tables merge recursively, scalars
/// and arrays replace.
fn merge_value(base: &mut toml::Value, over: toml::Value) {
    match (base, over) {
        (toml::Value::Table(b), toml::Value::Table(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(bv) => merge_value(bv, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (base, over) => *base = over,
    }
}
