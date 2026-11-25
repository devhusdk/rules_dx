use dx_testing::{env_files, path_text};

const FORBIDDEN: [&str; 2] = ["actions/cache", "cache-save"];

#[test]
fn nothing_under_github_uses_an_actions_cache() {
    let files = env_files("DX_GITHUB_FILES");
    assert!(
        files
            .iter()
            .any(|file| path_text(file).ends_with("workflows/ci.yml")),
        "DX_GITHUB_FILES does not hold .github/workflows/ci.yml"
    );
    let mut found = Vec::new();
    for file in &files {
        let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            for needle in FORBIDDEN {
                if line.contains(needle) {
                    found.push(format!("{}:{}: {needle}", file.display(), index + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "remove the GitHub Actions cache use:\n{}",
        found.join("\n")
    );
}
