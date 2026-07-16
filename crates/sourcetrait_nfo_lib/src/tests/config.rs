use crate::*;

/// A fresh, unique temp directory (no external tempfile dep).
fn temp_dir() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("nfo_cfg_test_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn write_defaults_writes_both_templates() {
    let root = temp_dir();
    let paths = Config::write_defaults_to(&root).unwrap();

    // Config template.
    let cfg = root.join("profile").join("defaults").join("nfo.toml");
    assert!(paths.contains(&cfg), "config path returned: {paths:?}");
    let cfg_text = std::fs::read_to_string(&cfg).unwrap();
    assert!(cfg_text.contains("[render]"));
    assert!(cfg_text.contains("reference = \"unused\""));

    // Theme template - parses, and carries the unable-link terms (the indent
    // drives the cli's inline dest spacing).
    let theme = root.join("theme").join("defaults.toml");
    assert!(paths.contains(&theme), "theme path returned: {paths:?}");
    let theme_text = std::fs::read_to_string(&theme).unwrap();
    let table: toml::Table = toml::from_str(&theme_text).expect("default theme parses");
    assert!(table.contains_key("header"));
    assert!(table.contains_key("body"), "[body] section present");
    let dest = table
        .get("unable")
        .and_then(|v| v.get("link"))
        .and_then(|v| v.get("dest"))
        .and_then(|v| v.as_table())
        .expect("unable.link.dest present");
    assert!(dest.contains_key("margin"), "unable.link.dest.margin present");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_from_reads_a_profile_merged_over_defaults() {
    let root = temp_dir();
    let pdir = root.join("profile").join("mine");
    std::fs::create_dir_all(&pdir).unwrap();
    std::fs::write(pdir.join("nfo.toml"), "[render]\nreference = \"always\"\n").unwrap();

    let cfg = Config::load_from(&root, Some("mine")).unwrap();
    assert_eq!(cfg.render.reference, ReferenceMode::Always);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_from_missing_falls_back_to_defaults() {
    let root = temp_dir();
    let cfg = Config::load_from(&root, None).unwrap();
    assert_eq!(cfg.render.reference, ReferenceMode::Unused);
    // No theme on disk -> the built-in default palette.
    assert!(cfg.palette.contains_key("all"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_from_resolves_the_selected_theme_palette() {
    let root = temp_dir();
    std::fs::create_dir_all(root.join("theme")).unwrap();
    // A stand-in theme with only a `header` term; loading it must replace the
    // built-in palette wholesale (no `all` term here).
    std::fs::write(root.join("theme").join("defaults.toml"), "[header]\nfg = [1, 2, 3]\n").unwrap();

    let cfg = Config::load_from(&root, None).unwrap();
    assert!(cfg.palette.contains_key("header"));
    assert!(!cfg.palette.contains_key("all"), "on-disk theme replaced the built-in palette");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn invalid_config_is_an_error() {
    assert!(Config::from_toml("[render]\nreference = \"bogus\"\n").is_err());
}

/// A theme that turns md on globally renders inline markers (the raw output
/// carries the literal `**` / `~~` marker runs).
#[test]
fn md_render_on_shows_inline_markers() {
    let root = temp_dir();
    std::fs::create_dir_all(root.join("theme")).unwrap();
    std::fs::write(root.join("theme").join("defaults.toml"), "[all]\nmd.render = true\n").unwrap();

    let cfg = Config::load_from(&root, None).unwrap();
    let out = render_markdown_ansi("**b** ~~s~~\n", 40, &cfg, ComponentKind::Cli);
    assert!(out.contains("**"), "strong markers with md on:\n{out}");
    assert!(out.contains("~~"), "strikethrough markers with md on:\n{out}");

    std::fs::remove_dir_all(&root).ok();
}

/// Full config pipeline: a hardcoded `plain_dark` profile + theme must render
/// everything white-on-black, with markup (`md.render`) and the always-listed
/// reference. The expected string IS the raw ANSI - a white-on-black terminal.
#[test]
fn plain_dark_profile_renders_raw_white_on_black() {
    let root = temp_dir();

    let pdir = root.join("profile").join("plain_dark");
    std::fs::create_dir_all(&pdir).unwrap();
    std::fs::write(
        pdir.join("nfo.toml"),
        "theme = \"plain_dark\"\n[render]\nreference = \"always\"\n",
    )
    .unwrap();

    std::fs::create_dir_all(root.join("theme")).unwrap();
    std::fs::write(
        root.join("theme").join("plain_dark.toml"),
        "[body]\nmargin = [1, 0, 1, 2]\njustify = \"left\"\n\n\
         [all]\nfg = [255, 255, 255]\nbg = [0, 0, 0]\nstyle = []\n\
         md.render = true\nmd.fg = [255, 255, 255]\nmd.bg = [0, 0, 0]\nmd.style = []\n",
    )
    .unwrap();

    let cfg = Config::load_from(&root, Some("plain_dark")).unwrap();
    let raw = render_markdown_ansi("# Hi\n\n[r]: x.md\n", 40, &cfg, ComponentKind::Cli);

    // Every run is white (255,255,255) on black (0,0,0): the `#` markup (via
    // md.render), the heading text, and the always-listed reference. Body margin
    // [1,0,1,2] -> one top blank line, two left cols, one bottom blank line.
    let wb = |s: &str| format!("\u{1b}[38;2;255;255;255;48;2;0;0;0m{s}\u{1b}[0m");
    let expected = format!("\n  {}{}{}\n\n  {}\n\n", wb("#"), wb(" "), wb("Hi"), wb("[r]: x.md"));
    assert_eq!(raw, expected, "plain_dark raw output");

    std::fs::remove_dir_all(&root).ok();
}
