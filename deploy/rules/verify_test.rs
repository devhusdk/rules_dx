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

fn resources_case() {
    let plain = std::env::var("DX_RESOURCES_PLAIN").expect("DX_RESOURCES_PLAIN must list files");
    let processed =
        std::env::var("DX_RESOURCES_PROCESSED").expect("DX_RESOURCES_PROCESSED must list files");
    let by_base = |paths: &str| {
        let mut found = std::collections::BTreeMap::new();
        for rel in paths.split_whitespace() {
            let path = dx_testing::resolve_runfiles(rel);
            let base = path
                .file_name()
                .expect("staged file must have a name")
                .to_string_lossy()
                .into_owned();
            found.insert(base, path);
        }
        found
    };
    let plain_files = by_base(&plain);
    let processed_files = by_base(&processed);
    let logo = std::fs::read_to_string(&plain_files["logo.txt"]).expect("read staged logo");
    assert_eq!(logo, "dx resources demo logo v1\n");
    let sprite = std::fs::read_to_string(&plain_files["sprite.bin"]).expect("read staged sprite");
    assert_eq!(sprite, "dx-generated-sprite-v1");
    let staged_logo =
        std::fs::read_to_string(&processed_files["logo.txt"]).expect("read processed logo");
    assert_eq!(
        staged_logo,
        "DX-RESOURCES-DEMO-V1\ndx resources demo logo v1\n"
    );
    let staged_sprite =
        std::fs::read_to_string(&processed_files["sprite.bin"]).expect("read processed sprite");
    assert_eq!(
        staged_sprite,
        "DX-RESOURCES-DEMO-V1\ndx-generated-sprite-v1"
    );
}

#[test]
fn deploy_verify() {
    let mode = std::env::var("DX_VERIFY_MODE").expect("DX_VERIFY_MODE must select the harness");
    match mode.as_str() {
        "archive" => archive_case(),
        "github" => github_case(),
        "resources" => resources_case(),
        other => panic!("unknown DX_VERIFY_MODE: {other}"),
    }
}
