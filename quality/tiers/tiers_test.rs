use std::collections::BTreeSet;
use std::path::Path;

const NO_FORMATTER: &[&str] = &[
    "cfg",
    "clang-format",
    "clang-tidy",
    "cmakelintrc",
    "conf",
    "config",
    "editorconfig",
    "exs",
    "globalconfig",
    "hcl",
    "ini",
    "neon",
    "perlcriticrc",
    "perltidyrc",
    "psd1",
    "shellcheckrc",
    "txt",
    "xml",
];

const EXEMPT: &[&str] = &[
    "quality/tiers/detekt/regen.sh",
    "quality/tiers/ruff/strict.toml",
    "quality/tiers/selene/strict.toml",
    "quality/tiers/vale/styles/",
];

fn paths(env: &str) -> BTreeSet<String> {
    std::env::var(env)
        .unwrap_or_else(|_| panic!("{env} must list runfiles"))
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn tier_files() -> BTreeSet<String> {
    paths("DX_TIER_FILES")
        .into_iter()
        .filter(|path| !EXEMPT.iter().any(|exempt| path.starts_with(exempt)))
        .collect()
}

fn is_tier(path: &str) -> bool {
    let name = Path::new(path).file_name().unwrap().to_string_lossy();
    name.starts_with("standard.") || name.starts_with("strict.")
}

#[test]
fn no_tier_file_names_another_tier() {
    let files = tier_files();
    for path in files.iter().filter(|path| is_tier(path)) {
        let text = std::fs::read_to_string(dx_testing::resolve_runfiles(path)).expect("read tier");
        let dir = Path::new(path).parent().unwrap();
        for sibling in files
            .iter()
            .filter(|other| is_tier(other) && *other != path)
        {
            let sibling = Path::new(sibling);
            if sibling.parent() != Some(dir) {
                continue;
            }
            let name = sibling.file_name().unwrap().to_string_lossy();
            assert!(!text.contains(name.as_ref()), "{path} names {name}");
        }
    }
}

#[test]
fn every_tier_file_is_in_a_corpus_or_has_no_formatter() {
    let corpora = paths("DX_TIER_CORPORA");
    let missing: Vec<String> = tier_files()
        .into_iter()
        .filter(|path| !corpora.contains(path))
        .filter(|path| {
            let ext = path.rsplit_once('.').map_or("", |(_, ext)| ext);
            !NO_FORMATTER.contains(&ext)
        })
        .collect();
    assert!(
        missing.is_empty(),
        "add these to quality/tiers/corpora.bzl: {missing:?}"
    );
}

#[test]
fn clippy_tiers_end_with_deny_warnings() {
    for path in tier_files()
        .iter()
        .filter(|path| path.starts_with("quality/tiers/clippy/") && is_tier(path))
    {
        let text = std::fs::read_to_string(dx_testing::resolve_runfiles(path)).expect("read tier");
        assert_eq!(text.lines().last(), Some("-Dwarnings"), "{path}");
    }
}
