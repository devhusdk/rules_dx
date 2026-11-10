use dx_testing::read_runfiles;

const MODULE: &str = "MODULE.bazel";
const ROOT_BUILD: &str = "BUILD.bazel";
const EXCLUDE: &str = "# gazelle:exclude ";

/// Root excludes that still hide a dx_crates package.
const ALLOWED: &[&str] = &["cli/depcheck", "deploy/rules", "core/quality/tiers"];

fn root_excludes() -> Vec<String> {
    read_runfiles(ROOT_BUILD)
        .lines()
        .filter_map(|line| line.strip_prefix(EXCLUDE))
        .map(|path| path.trim().trim_end_matches('/').to_owned())
        .collect()
}

fn dx_crates_manifest_dirs() -> Vec<String> {
    let text = read_runfiles(MODULE);
    let lines: Vec<&str> = text.lines().collect();
    let name = lines
        .iter()
        .position(|line| line.trim() == "name = \"dx_crates\",")
        .unwrap_or_else(|| panic!("{MODULE} declares no dx_crates from_cargo"));
    let start = lines[name..]
        .iter()
        .position(|line| line.trim_start().starts_with("manifests = ["))
        .map(|offset| name + offset)
        .unwrap_or_else(|| panic!("dx_crates sets no manifests"));
    lines[start + 1..]
        .iter()
        .take_while(|line| !line.trim_start().starts_with(']'))
        .filter_map(|line| {
            let label = line.trim().strip_prefix('"')?;
            let label = label[..label.find('"')?].strip_prefix("//")?;
            let dir = label.strip_suffix(":Cargo.toml")?;
            Some(dir.to_owned())
        })
        .collect()
}

fn hides(exclude: &str, dir: &str) -> bool {
    dir == exclude || dir.starts_with(&format!("{exclude}/"))
}

#[test]
fn root_excludes_do_not_hide_dx_crates_packages() {
    let excludes = root_excludes();
    let dirs = dx_crates_manifest_dirs();
    assert!(!dirs.is_empty(), "dx_crates manifests must parse");
    assert!(!excludes.is_empty(), "{ROOT_BUILD} excludes must parse");
    let offenders: Vec<String> = excludes
        .iter()
        .flat_map(|exclude| {
            dirs.iter()
                .filter(|dir| hides(exclude, dir))
                .map(move |dir| format!("gazelle:exclude {exclude} hides //{dir}:Cargo.toml"))
        })
        .filter(|pair| {
            !ALLOWED
                .iter()
                .any(|allowed| pair.contains(&format!("exclude {allowed} ")))
        })
        .collect();
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

#[test]
fn allowlist_entries_still_hide_a_package() {
    let excludes = root_excludes();
    let dirs = dx_crates_manifest_dirs();
    for allowed in ALLOWED {
        assert!(
            excludes.iter().any(|exclude| exclude == allowed)
                && dirs.iter().any(|dir| hides(allowed, dir)),
            "{allowed} no longer hides a dx_crates package and must leave ALLOWED"
        );
    }
}
