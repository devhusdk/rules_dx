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

fn cloudflare_case() {
    let prog = data("DX_CLOUDFLARE_PROG");
    let production = data("DX_CLOUDFLARE_PRODUCTION_PROG");
    let token = "dx-test-token-123";
    let env = [
        ("CLOUDFLARE_API_TOKEN", token),
        ("AMBIENT_SECRET", "ambient"),
        ("DX_PROFILE", "release"),
    ];

    let preview = dx_testing::run(&prog, &[], &env).expect("preview deploy must execute");
    let out = preview.combined();
    assert!(preview.status.success(), "preview deploy failed\n{out}");
    for want in [
        "deployed dx-demo (branch feature/demo, preview, profile: release)",
        "  account: 0123456789abcdef0123456789abcdef",
        "  project: dx-demo",
        "  branch: feature/demo",
        "  intent: preview",
        "  files: 3 (",
        "  deployment: dep-clean",
        "  url: https://feature-demo.dx-demo.pages.dev",
    ] {
        assert!(out.contains(want), "missing '{want}'\n{out}");
    }
    assert!(!out.contains(token), "token leaked\n{out}");

    let live = dx_testing::run(&production, &[], &[("CLOUDFLARE_API_TOKEN", token)])
        .expect("production deploy must execute");
    let out = live.combined();
    assert!(live.status.success(), "production deploy failed\n{out}");
    assert!(out.contains("  intent: production"), "{out}");
    assert!(out.contains("url: https://main.dx-demo.pages.dev"), "{out}");

    let pinned = data("DX_CLOUDFLARE_DEFAULT_PROG");
    let real = dx_testing::run(&pinned, &[], &[("CLOUDFLARE_API_TOKEN", "bogus-token")])
        .expect("pinned wrangler run must execute");
    let out = real.combined();
    assert!(!real.status.success(), "{out}");
    assert!(
        !out.contains("FATAL"),
        "pinned wrangler did not start\n{out}"
    );
    assert!(out.contains("wrangler failed with exit"), "{out}");
    assert!(!out.contains("bogus-token"), "token leaked\n{out}");

    let missing = dx_testing::run(&prog, &[], &[("CLOUDFLARE_API_TOKEN", "")])
        .expect("missing token run must execute");
    assert!(!missing.status.success());
    assert!(
        missing.stderr.contains("CLOUDFLARE_API_TOKEN is not set"),
        "{}",
        missing.stderr
    );

    let denied = dx_testing::run(&prog, &[], &[("CLOUDFLARE_API_TOKEN", "bad-token")])
        .expect("denied run must execute");
    assert_eq!(denied.status.code(), Some(9), "{}", denied.combined());
    assert!(
        !denied.combined().contains("bad-token"),
        "{}",
        denied.combined()
    );
    assert!(denied.stderr.contains("[redacted]"), "{}", denied.stderr);

    for removed in [
        dx_testing::run(&prog, &["--dry-run"], &env).expect("removed flag must execute"),
        dx_testing::run(
            &prog,
            &[],
            &[
                ("CLOUDFLARE_PAGES_DRY_RUN", "1"),
                ("CLOUDFLARE_API_TOKEN", token),
            ],
        )
        .expect("removed env must execute"),
    ] {
        assert!(!removed.status.success(), "{}", removed.combined());
        assert!(removed.stdout.is_empty(), "{}", removed.combined());
        assert!(
            removed.stderr.contains("takes no arguments")
                || removed
                    .stderr
                    .contains("CLOUDFLARE_PAGES_DRY_RUN is not supported"),
            "{}",
            removed.stderr
        );
    }
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

fn oci_case() {
    let prog = data("DX_OCI_PROG");
    let outdir = dx_testing::mkscratch("oci-demo").expect("scratch");
    let run = dx_testing::run(&prog, &[outdir.to_string_lossy().as_ref()], &[])
        .expect("oci deploy must execute");
    let out = run.combined();
    assert!(run.status.success(), "oci deploy failed\n{out}");
    let layout = out
        .lines()
        .find_map(|line| line.strip_prefix("  layout: "))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("no layout line\n{out}"));
    let blobs = layout.join("blobs").join("sha256");
    for entry in std::fs::read_dir(&blobs).expect("read blobs") {
        let path = entry.expect("blob entry").path();
        let name = path
            .file_name()
            .expect("blob name")
            .to_string_lossy()
            .into_owned();
        assert_eq!(dx_testing::sha256_file(&path).expect("hash blob"), name);
    }
    let blob = |digest: &dx_testing::serde_json::Value| {
        let digest = digest.as_str().expect("digest string");
        let hex = digest.strip_prefix("sha256:").expect("sha256 digest");
        dx_testing::read_json(&blobs.join(hex)).expect("blob json")
    };
    let index = dx_testing::read_json(&layout.join("index.json")).expect("index");
    assert_eq!(
        index["manifests"][0]["annotations"]["org.opencontainers.image.ref.name"],
        "ghcr.io/oci_demo:latest"
    );
    let manifest = blob(&index["manifests"][0]["digest"]);
    let layers = manifest["layers"].as_array().expect("layers");
    assert_eq!(layers.len(), 2, "{manifest}");
    for layer in layers {
        let hex = layer["digest"].as_str().expect("layer digest");
        assert!(blobs.join(&hex[7..]).is_file(), "{hex}");
    }
    let config = blob(&manifest["config"]["digest"]);
    assert_eq!(
        config["config"]["Entrypoint"],
        dx_testing::serde_json::json!(["/app/hello"])
    );
    assert_eq!(config["os"], "linux");
    assert_eq!(config["architecture"], "amd64");
    assert_eq!(
        config["rootfs"]["diff_ids"].as_array().map(Vec::len),
        Some(2),
        "{config}"
    );
}

#[test]
fn deploy_verify() {
    let mode = std::env::var("DX_VERIFY_MODE").expect("DX_VERIFY_MODE must select the harness");
    match mode.as_str() {
        "archive" => archive_case(),
        "cloudflare" => cloudflare_case(),
        "github" => github_case(),
        "oci" => oci_case(),
        "resources" => resources_case(),
        other => panic!("unknown DX_VERIFY_MODE: {other}"),
    }
}
