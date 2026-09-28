use std::path::PathBuf;

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn archive_case() {
    let tarball = data("DX_ARCHIVE_TARBALL");
    let checksum = data("DX_ARCHIVE_CHECKSUM");
    let member =
        std::env::var("DX_ARCHIVE_MEMBER").expect("DX_ARCHIVE_MEMBER must name the member");

    let listing = dx_testing::run(
        &PathBuf::from("tar"),
        &["-tzf", tarball.to_string_lossy().as_ref()],
        &[],
    )
    .expect("tar must execute");
    assert!(
        listing.status.success(),
        "cannot list {}\n{}",
        tarball.display(),
        listing.combined()
    );
    assert!(
        listing.stdout.lines().any(|line| line == member),
        "archive member '{member}' not found in {}\ntarball contents:\n{}",
        tarball.display(),
        listing.stdout
    );

    let checksum_text = std::fs::read_to_string(&checksum).expect("read checksum");
    let expected = checksum_text
        .split_whitespace()
        .next()
        .expect("checksum field");
    let digest = dx_testing::run(
        &PathBuf::from("sha256sum"),
        &[tarball.to_string_lossy().as_ref()],
        &[],
    )
    .expect("sha256sum must execute");
    assert!(digest.status.success(), "cannot hash {}", tarball.display());
    let actual = digest
        .stdout
        .split_whitespace()
        .next()
        .expect("digest field");
    assert_eq!(
        expected,
        actual,
        "checksum mismatch for {}\n  expected: {expected}\n  actual:   {actual}",
        tarball.display()
    );
}

fn github_case() {
    let prog = data("DX_GITHUB_PROG");
    let want_tag = std::env::var("DX_GITHUB_TAG").expect("DX_GITHUB_TAG must name the tag");
    let assets = std::env::var("DX_GITHUB_ASSETS").expect("DX_GITHUB_ASSETS must list assets");

    let run =
        dx_testing::run(&prog, &[], &[("GH_RELEASE_DRY_RUN", "1")]).expect("dry run must execute");
    let out = run.combined();
    assert!(
        out.lines()
            .any(|line| line.trim() == format!("tag: {want_tag}")),
        "github tag line 'tag: {want_tag}' not found\n{out}"
    );
    for base in assets.split_whitespace() {
        assert!(
            out.contains(&format!("asset: {base} ")),
            "github asset '{base}' not reported\n{out}"
        );
    }
    assert!(
        out.contains(&format!("gh release create {want_tag} ")),
        "github would-run command missing 'gh release create {want_tag}'\n{out}"
    );
    assert!(
        out.lines()
            .any(|line| line.ends_with("--draft --verify-tag")),
        "github would-run command missing draft-only flags '--draft --verify-tag'\n{out}"
    );
}

#[test]
fn deploy_verify() {
    let mode = std::env::var("DX_VERIFY_MODE").expect("DX_VERIFY_MODE must select the harness");
    match mode.as_str() {
        "archive" => archive_case(),
        "github" => github_case(),
        other => panic!("unknown DX_VERIFY_MODE: {other}"),
    }
}
