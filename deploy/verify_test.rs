use dx_testing::http::{Reply, RequestLog, requests, serve as serve_with};
use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn run_deploy(
    prog: &Path,
    config_var: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> std::io::Result<dx_testing::Run> {
    let config = data(config_var);
    let config = config.to_string_lossy();
    let mut env = env.to_vec();
    env.push(("DX_DEPLOY_CONFIG", config.as_ref()));
    dx_testing::run(prog, args, &env)
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

fn homebrew_case() {
    let prog = data("DX_HOMEBREW_PROG");
    let outdir = dx_testing::mkscratch("homebrew-out").expect("scratch outdir");
    let run = run_deploy(
        &prog,
        "DX_HOMEBREW_CONFIG",
        &[outdir.to_str().expect("utf8 outdir")],
        &[],
    )
    .expect("homebrew launcher must execute");
    let out = run.combined();
    assert_eq!(run.status.code(), Some(0), "{out}");
    assert!(
        out.contains("homebrew_tap_deploy: staged formula, pushing nothing:"),
        "{out}"
    );
    let formula = std::fs::read_to_string(
        outdir
            .join("homebrew_demo-tap")
            .join("Formula")
            .join("homebrew-demo.rb"),
    )
    .expect("staged formula");
    assert!(
        formula.starts_with("class HomebrewDemo < Formula\n"),
        "{formula}"
    );
    for want in [
        "version \"1.2.3\"",
        "on_macos do",
        "on_linux do",
        "releases/download/v1.2.3/homebrew_demo-darwin-arm64.tar.gz",
        "releases/download/v1.2.3/homebrew_demo-linux-x86_64.tar.gz",
    ] {
        assert!(formula.contains(want), "formula missing {want}\n{formula}");
    }
    for var in ["DX_HOMEBREW_MAC", "DX_HOMEBREW_LINUX"] {
        let digest = dx_testing::sha256_file(&data(var)).expect("hash asset");
        assert!(
            formula.contains(&format!("sha256 \"{digest}\"")),
            "{var} digest {digest} missing\n{formula}"
        );
    }
}

fn github_case() {
    let prog = data("DX_GITHUB_PROG");
    let want_tag = std::env::var("DX_GITHUB_TAG").expect("DX_GITHUB_TAG must name the tag");
    let assets = std::env::var("DX_GITHUB_ASSETS").expect("DX_GITHUB_ASSETS must list assets");

    let run =
        run_deploy(&prog, "DX_GITHUB_CONFIG", &[], &[]).expect("github launcher must execute");
    let out = run.combined();
    assert!(
        out.lines()
            .any(|line| line.trim() == "github_deploy: staged draft release, publishing nothing:"),
        "github staged header not found\n{out}"
    );
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
    if let Ok(want) = std::env::var("DX_GITHUB_SBOM_PACKAGES") {
        assert!(
            out.lines()
                .any(|line| line.trim() == format!("sbom: app.spdx.json {want} packages")),
            "github sbom package count {want} not reported\n{out}"
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

    let bare = dx_testing::run(&prog, &[], &[]).expect("unconfigured run must execute");
    assert_eq!(bare.status.code(), Some(2), "{}", bare.combined());
    assert!(
        bare.stderr
            .contains("DX_DEPLOY_CONFIG not set: run with bazel run or dx deploy"),
        "{}",
        bare.stderr
    );
}

fn cloudflare_case() {
    let prog = data("DX_CLOUDFLARE_PROG");
    let production = data("DX_CLOUDFLARE_PRODUCTION_PROG");
    let token = "dx-test-token-123";
    let env = [
        ("DX_DEPLOY_LIVE", "1"),
        ("CLOUDFLARE_PUBLISH_APPROVED", "1"),
        ("CLOUDFLARE_API_TOKEN", token),
        ("AMBIENT_SECRET", "ambient"),
        ("DX_BUILD_MODE", "release"),
    ];

    let preview =
        run_deploy(&prog, "DX_CLOUDFLARE_CONFIG", &[], &env).expect("preview deploy must execute");
    let out = preview.combined();
    assert!(preview.status.success(), "preview deploy failed\n{out}");
    for want in [
        "deployed dx-demo (branch feature/demo, preview, build_mode: release)",
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

    let live = run_deploy(
        &production,
        "DX_CLOUDFLARE_PRODUCTION_CONFIG",
        &[],
        &[
            ("DX_DEPLOY_LIVE", "1"),
            ("CLOUDFLARE_PUBLISH_APPROVED", "1"),
            ("CLOUDFLARE_API_TOKEN", token),
        ],
    )
    .expect("production deploy must execute");
    let out = live.combined();
    assert!(live.status.success(), "production deploy failed\n{out}");
    assert!(out.contains("  intent: production"), "{out}");
    assert!(out.contains("url: https://main.dx-demo.pages.dev"), "{out}");

    let pinned = data("DX_CLOUDFLARE_DEFAULT_PROG");
    let real = run_deploy(
        &pinned,
        "DX_CLOUDFLARE_DEFAULT_CONFIG",
        &[],
        &[
            ("DX_DEPLOY_LIVE", "1"),
            ("CLOUDFLARE_PUBLISH_APPROVED", "1"),
            ("CLOUDFLARE_API_TOKEN", "bogus-token"),
        ],
    )
    .expect("pinned wrangler run must execute");
    let out = real.combined();
    assert!(!real.status.success(), "{out}");
    assert!(
        !out.contains("FATAL"),
        "pinned wrangler did not start\n{out}"
    );
    assert!(out.contains("wrangler failed with exit"), "{out}");
    assert!(!out.contains("bogus-token"), "token leaked\n{out}");

    let missing = run_deploy(
        &prog,
        "DX_CLOUDFLARE_CONFIG",
        &[],
        &[
            ("DX_DEPLOY_LIVE", "1"),
            ("CLOUDFLARE_PUBLISH_APPROVED", "1"),
            ("CLOUDFLARE_API_TOKEN", ""),
        ],
    )
    .expect("missing token run must execute");
    assert!(!missing.status.success());
    assert!(
        missing.stderr.contains("live upload needs CLOUDFLARE_API_TOKEN"),
        "{}",
        missing.stderr
    );

    let denied = run_deploy(
        &prog,
        "DX_CLOUDFLARE_CONFIG",
        &[],
        &[
            ("DX_DEPLOY_LIVE", "1"),
            ("CLOUDFLARE_PUBLISH_APPROVED", "1"),
            ("CLOUDFLARE_API_TOKEN", "bad-token"),
        ],
    )
    .expect("denied run must execute");
    assert_eq!(denied.status.code(), Some(9), "{}", denied.combined());
    assert!(
        !denied.combined().contains("bad-token"),
        "{}",
        denied.combined()
    );
    assert!(denied.stderr.contains("[redacted]"), "{}", denied.stderr);

    let checked = run_deploy(
        &prog,
        "DX_CLOUDFLARE_CONFIG",
        &[],
        &[
            ("CLOUDFLARE_API_TOKEN", token),
            ("DX_BUILD_MODE", "release"),
        ],
    )
    .expect("check run must execute");
    let out = checked.combined();
    assert!(checked.status.success(), "check run failed\n{out}");
    assert!(
        checked.stdout.contains(
            "checked dx-demo (branch feature/demo, preview, build_mode: release); nothing uploaded"
        ),
        "{out}"
    );
    assert!(!out.contains("deployment:"), "{out}");
    assert!(!out.contains(token), "token leaked\n{out}");

    let unapproved = run_deploy(
        &prog,
        "DX_CLOUDFLARE_CONFIG",
        &[],
        &[("DX_DEPLOY_LIVE", "1"), ("CLOUDFLARE_API_TOKEN", token)],
    )
    .expect("unapproved run must execute");
    assert!(!unapproved.status.success(), "{}", unapproved.combined());
    assert!(
        unapproved.stderr.contains("CLOUDFLARE_PUBLISH_APPROVED=1"),
        "{}",
        unapproved.stderr
    );
    assert!(unapproved.stdout.is_empty(), "{}", unapproved.combined());

    for removed in [
        run_deploy(&prog, "DX_CLOUDFLARE_CONFIG", &["--dry-run"], &env)
            .expect("removed flag must execute"),
    ] {
        assert!(!removed.status.success(), "{}", removed.combined());
        assert!(removed.stdout.is_empty(), "{}", removed.combined());
        assert!(
            removed.stderr.contains("takes no arguments"),
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

fn helm_case() {
    let outdir = dx_testing::mkscratch("helm-demo").expect("scratch");
    let run = run_deploy(
        &data("DX_HELM_PROG"),
        "DX_HELM_CONFIG",
        &[outdir.to_string_lossy().as_ref()],
        &[],
    )
    .expect("helm_chart_deploy must execute");
    let out = run.combined();
    assert!(run.status.success(), "helm_chart_deploy failed\n{out}");
    let package = outdir
        .join("helm_demo-package")
        .join("dx-helm-demo-1.2.3.tgz");
    assert!(package.is_file(), "{out}");
    assert!(out.contains("staged dx-helm-demo-1.2.3.tgz"), "{out}");
}

fn rubygems_case() {
    let outdir = dx_testing::mkscratch("rubygems-demo").expect("scratch");
    let run = run_deploy(
        &data("DX_RUBYGEMS_PROG"),
        "DX_RUBYGEMS_CONFIG",
        &[outdir.to_string_lossy().as_ref()],
        &[("PATH", "/nonexistent")],
    )
    .expect("rubygems_deploy must execute");
    let out = run.combined();
    assert!(run.status.success(), "rubygems_deploy failed\n{out}");
    let gem = outdir.join("rubygems_demo-0.0.0.gem");
    assert!(gem.is_file(), "{out}");
    assert!(out.contains("staged rubygems_demo 0.0.0"), "{out}");
}

fn oci_stage(prog: &Path, config_var: &str, name: &str) -> PathBuf {
    let outdir = dx_testing::mkscratch(name).expect("scratch");
    let run = run_deploy(prog, config_var, &[outdir.to_string_lossy().as_ref()], &[])
        .expect("oci deploy must execute");
    let out = run.combined();
    assert!(run.status.success(), "oci deploy failed\n{out}");
    out.lines()
        .find_map(|line| line.strip_prefix("  layout: "))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("no layout line\n{out}"))
}

fn oci_case() {
    let layout = oci_stage(&data("DX_OCI_PROG"), "DX_OCI_CONFIG", "oci-demo");
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
        std::env::var("DX_OCI_REF").unwrap_or_else(|_| "ghcr.io/oci_demo:latest".to_owned())
    );
    let manifest = blob(&index["manifests"][0]["digest"]);
    if std::env::var_os("DX_OCI_BASE_PROG").is_some() {
        let base = oci_stage(
            &data("DX_OCI_BASE_PROG"),
            "DX_OCI_BASE_CONFIG",
            "oci-demo-base",
        );
        let base_index = dx_testing::read_json(&base.join("index.json")).expect("base index");
        let base_blobs = base.join("blobs").join("sha256");
        let digest = base_index["manifests"][0]["digest"]
            .as_str()
            .expect("base digest");
        let base_manifest =
            dx_testing::read_json(&base_blobs.join(&digest[7..])).expect("base manifest");
        assert_eq!(manifest["config"], base_manifest["config"]);
        assert_eq!(manifest["layers"], base_manifest["layers"]);
    }
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

fn serve(root: PathBuf) -> (String, RequestLog) {
    serve_with(move |target| {
        let path = target.split(['?', '#']).next().unwrap_or_default();
        let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        let mut file = root.clone();
        for segment in &segments {
            file.push(segment);
        }
        if path.ends_with('/') || file.is_dir() {
            file.push("index.html");
        }
        if segments.contains(&"..") || !file.is_file() {
            return None;
        }
        let content_type = if file.extension().is_some_and(|ext| ext == "html") {
            "text/html"
        } else {
            "application/octet-stream"
        };
        Some(Reply {
            content_type,
            body: std::fs::read(&file).unwrap_or_default(),
        })
    })
    .expect("start loopback server")
}

fn stage(
    var: &str,
    config_var: &str,
    house_name: &str,
    project: &str,
    target_name: &str,
) -> PathBuf {
    let prog = data(var);
    let outdir = dx_testing::mkscratch("pypi-out").expect("scratch outdir");
    let run = run_deploy(&prog, config_var, &[outdir.to_string_lossy().as_ref()], &[])
        .expect("pypi_deploy program must execute");
    assert!(run.status.success(), "{}", run.combined());
    let house = outdir.join(house_name);
    let index = house.join("simple-index");
    assert!(
        index.join(project).join("index.html").is_file(),
        "{}",
        run.combined()
    );
    assert!(!index.join(target_name).exists(), "{}", house.display());
    house
}

fn kept_env(command: &mut std::process::Command) {
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        let kept: Vec<PathBuf> = std::env::split_paths(&path)
            .filter(|dir| {
                let text = dir.to_string_lossy();
                !text.contains("Visual Studio") && !text.contains("Windows Kits")
            })
            .collect();
        command.env("PATH", std::env::join_paths(kept).expect("join PATH"));
    }
    for key in ["SYSTEMROOT", "TEMP", "TMP", "HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}

fn install_output(url: &str, spelling: &str) -> (std::process::Output, PathBuf) {
    let uv = data("DX_PYPI_UV");
    let python = data("DX_PYPI_PYTHON");
    let cache = dx_testing::mkscratch("pypi-uv-cache").expect("scratch cache");
    let target = dx_testing::mkscratch("pypi-target").expect("scratch target");
    let mut command = std::process::Command::new(&uv);
    kept_env(&mut command);
    command
        .env("UV_CACHE_DIR", &cache)
        .env("UV_NO_CONFIG", "1")
        .env("UV_PYTHON_DOWNLOADS", "never")
        .args([
            "pip",
            "install",
            "--no-config",
            "--no-cache",
            "--no-build",
            "--no-deps",
            "--strict",
            "--python",
        ])
        .arg(&python)
        .arg("--default-index")
        .arg(format!("{url}/simple-index/"))
        .arg("--target")
        .arg(&target)
        .arg(spelling);
    let output = command.output().expect("uv must execute");
    (output, target)
}

fn install(url: &str, spelling: &str) -> PathBuf {
    let (output, target) = install_output(url, spelling);
    assert!(
        output.status.success(),
        "uv pip install {spelling} failed\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    target
}

fn run_module(target: &Path, module: &str) -> std::process::Output {
    let mut command = std::process::Command::new(data("DX_PYPI_PYTHON"));
    kept_env(&mut command);
    command.env("PYTHONPATH", target).args(["-m", module]);
    command.output().expect("python must execute")
}

fn check_module(target: &Path, module: &str, expected: &str) {
    let output = run_module(target, module);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected,
        "{module}"
    );
}

fn pypi_case() {
    let wheel = "dx_pypi_probe-1.0-py3-none-any.whl";
    let house = stage(
        "DX_PYPI_PROG",
        "DX_PYPI_CONFIG",
        "dx_pypi_probe-wheelhouse",
        "dx-pypi-probe",
        "dx_pypi_probe",
    );
    let (url, log) = serve(house.clone());
    for spelling in [
        "Dx_Pypi.Probe",
        "dx_pypi_probe",
        "DX.PYPI.PROBE",
        "dx--pypi__probe",
        "dx-pypi-probe",
    ] {
        let target = install(&url, spelling);
        check_module(&target, "dx_pypi_probe", "dx-pypi-probe ok");
    }
    let seen = requests(&log);
    assert!(
        seen.iter()
            .any(|path| path == "/simple-index/dx-pypi-probe/"),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|path| path.ends_with(&format!("/{wheel}"))),
        "{seen:?}"
    );
    let digest = dx_testing::sha256_file(&house.join(wheel)).expect("hash wheel");
    let index = std::fs::read_to_string(
        house
            .join("simple-index")
            .join("dx-pypi-probe")
            .join("index.html"),
    )
    .expect("read index");
    assert!(index.contains(&format!("#sha256={digest}")), "{index}");

    let legacy = "Dx_Pypi.Probe-1.0-py3-none-any.whl";
    let house = stage(
        "DX_PYPI_RENAMED_PROG",
        "DX_PYPI_RENAMED_CONFIG",
        "pypi_probe_release-wheelhouse",
        "dx-pypi-probe",
        "pypi_probe_release",
    );
    assert!(house.join(legacy).is_file(), "{}", house.display());
    let (url, log) = serve(house);
    let target = install(&url, "Dx_Pypi.Probe");
    check_module(&target, "dx_pypi_probe", "dx-pypi-probe ok");
    let seen = requests(&log);
    assert!(
        seen.iter()
            .any(|path| path.ends_with(&format!("/{legacy}"))),
        "{seen:?}"
    );

    let prog = data("DX_PYPI_MISMATCH_PROG");
    let outdir = dx_testing::mkscratch("pypi-mismatch").expect("scratch outdir");
    let run = run_deploy(
        &prog,
        "DX_PYPI_MISMATCH_CONFIG",
        &[outdir.to_string_lossy().as_ref()],
        &[],
    )
    .expect("pypi_deploy program must execute");
    assert!(!run.status.success(), "{}", run.combined());
    assert!(
        run.stderr.contains(
            "pypi_deploy: wheel dx_pypi_probe-1.0-py3-none-any.whl is dx-pypi-probe 1.0 but sdist dx_other-1.0.tar.gz is dx-other 1.0"
        ),
        "{}",
        run.stderr
    );

    typed_case();
}

fn typed_case() {
    let house = stage(
        "DX_TYPED_PROG",
        "DX_TYPED_CONFIG",
        "dx_typed_probe-wheelhouse",
        "dx-typed-probe",
        "dx_typed_probe",
    );
    let (url, _log) = serve(house);
    let target = install(&url, "dx_typed_probe");
    check_module(&target, "dx_ns.typed_probe", "dx-typed-probe hello");
    let package = target.join("dx_ns").join("typed_probe");
    assert!(package.join("py.typed").is_file());
    assert!(package.join("__init__.pyi").is_file());
    assert!(!target.join("dx_ns").join("__init__.py").exists());

    let house = stage(
        "DX_TYPED_NODATA_PROG",
        "DX_TYPED_NODATA_CONFIG",
        "typed_probe_nodata-wheelhouse",
        "dx-typed-probe",
        "typed_probe_nodata",
    );
    let (url, _log) = serve(house);
    let target = install(&url, "dx_typed_probe");
    let output = run_module(&target, "dx_ns.typed_probe");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("dx-typed-probe: missing data/greeting.txt"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn probe(python: &Path, code: &str) -> String {
    let output = std::process::Command::new(python)
        .args(["-I", "-c", code])
        .output()
        .expect("python must execute");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn no_compilers(command: &mut std::process::Command) {
    if !cfg!(windows) {
        let empty = dx_testing::mkscratch("pypi-empty-path").expect("scratch empty path");
        command.env("PATH", empty);
    }
}

fn import_from(target: &Path, cwd: &Path) -> std::process::Output {
    let mut command = std::process::Command::new(data("DX_PYPI_PYTHON"));
    kept_env(&mut command);
    no_compilers(&mut command);
    command.current_dir(cwd).env("PYTHONPATH", target).args([
        "-c",
        "import dx_native_probe; print(dx_native_probe.greet())",
    ]);
    command.output().expect("python must execute")
}

fn native_files(root: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).expect("read dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            native_files(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn pypi_native_case() {
    let python = data("DX_PYPI_PYTHON");
    let version = probe(
        &python,
        "import sys; print('cp%d%d' % sys.version_info[:2])",
    );
    let suffix = probe(
        &python,
        "import sysconfig; print(sysconfig.get_config_var('EXT_SUFFIX'))",
    );
    assert_eq!(version, "cp312", "native probe wheel is built for cp312");
    let (platform, ext_name) = if cfg!(windows) {
        ("win_amd64", "pyd")
    } else {
        ("linux_x86_64", "so")
    };
    let wheel = format!("dx_native_probe-1.0-{version}-{version}-{platform}.whl");
    let house = stage(
        "DX_NATIVE_PROG",
        "DX_NATIVE_CONFIG",
        "dx_native_probe-wheelhouse",
        "dx-native-probe",
        "dx_native_probe",
    );
    assert!(house.join(&wheel).is_file(), "{}", house.display());
    let (url, _log) = serve(house.clone());
    let target = install(&url, "dx_native_probe");

    let mut files = Vec::new();
    native_files(&target, &mut files);
    let ext: Vec<&PathBuf> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == ext_name))
        .collect();
    assert_eq!(ext.len(), 1, "{files:?}");
    assert_eq!(
        ext[0],
        &target
            .join("dx_native_probe")
            .join(format!("_core{suffix}"))
    );
    for path in &files {
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        assert!(!["h", "a", "o"].contains(&extension), "{path:?}");
    }

    for typed in ["py.typed", "__init__.pyi"] {
        assert!(
            target.join("dx_native_probe").join(typed).is_file(),
            "{typed} missing from {files:?}"
        );
    }
    if cfg!(target_os = "linux") {
        let core = ext[0];
        let dynamic = readelf_dynamic(core);
        println!("readelf -d {}\n{dynamic}", core.display());
        println!("readelf -V\n{}", readelf_output("-V", core));
        assert!(!dynamic.contains("libpython"), "{dynamic}");
        for line in dynamic.lines().filter(|line| line.contains("RUNPATH")) {
            assert!(
                !line.contains("bazel-out") && !line.contains("[/"),
                "{line}"
            );
        }
    }

    let cwd = dx_testing::mkscratch("pypi-native-cwd").expect("scratch cwd");
    let output = import_from(&target, &cwd);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "dx-native-probe native"
    );
    let moved = target.with_file_name(format!(
        "{}-moved",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("t")
    ));
    std::fs::rename(&target, &moved).expect("rename install dir");
    let output = import_from(&moved, &cwd);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    check_module(&moved, "dx_native_probe", "dx-native-probe native");

    let digest = dx_testing::sha256_file(&house.join(&wheel)).expect("hash wheel");
    println!("python {version} wheel {wheel} sha256 {digest}");

    let outdir = dx_testing::mkscratch("pypi-wrongabi").expect("scratch outdir");
    let run = run_deploy(
        &data("DX_NATIVE_WRONGABI_PROG"),
        "DX_NATIVE_WRONGABI_CONFIG",
        &[outdir.to_string_lossy().as_ref()],
        &[],
    )
    .expect("pypi_deploy program must execute");
    assert!(!run.status.success(), "{}", run.combined());
    assert!(
        run.stderr
            .contains("is tagged cp311 but contains dx_native_probe/_core"),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("built for cp312"), "{}", run.stderr);

    let house = stage(
        "DX_NATIVE_NOCORE_PROG",
        "DX_NATIVE_NOCORE_CONFIG",
        "native_probe_nocore-wheelhouse",
        "dx-native-probe",
        "native_probe_nocore",
    );
    let (url, _log) = serve(house);
    let (output, target) = install_output(&url, "dx_native_probe");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = import_from(&target, &cwd);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("_core"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn archiver_imports(
    members: &[(String, PathBuf)],
    check: &str,
    provides: &[&str],
) -> std::process::Output {
    let out = dx_testing::mkscratch("pypi-imports")
        .expect("scratch imports")
        .join("ok");
    let mut command = std::process::Command::new(data("DX_ARCHIVER"));
    command.arg("--imports").arg(out).args(["--check", check]);
    for name in provides {
        command.args(["--provides", name]);
    }
    for (name, path) in members {
        command.arg(format!("{name}={}", path.display()));
    }
    command.output().expect("archiver must execute")
}

fn installed_binaries(target: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    native_files(&target.join("dx_native_probe"), &mut found);
    found
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_owned();
            let lower = name.to_ascii_lowercase();
            (lower.ends_with(".dll") || lower.ends_with(".pyd")).then_some((name, path))
        })
        .collect()
}

fn pypi_native_msvc_case() {
    let core = |members: &[(String, PathBuf)]| {
        members
            .iter()
            .find(|(name, _)| name.ends_with(".pyd"))
            .map(|(name, _)| name.clone())
            .expect("wheel must contain the extension")
    };
    let house = stage(
        "DX_MSVC_PROG",
        "DX_MSVC_CONFIG",
        "native_probe_msvc-wheelhouse",
        "dx-native-probe",
        "native_probe_msvc",
    );
    let (url, _log) = serve(house);
    let target = install(&url, "dx_native_probe");
    let members = installed_binaries(&target);
    println!("wheel binaries:");
    for (name, path) in &members {
        println!("  {name} {}", path.display());
    }
    for dll in ["vcruntime140.dll", "msvcp140.dll"] {
        assert!(
            members.iter().any(|(name, _)| name.eq_ignore_ascii_case(dll)),
            "{dll} missing from {members:?}"
        );
    }
    let notice = target.join("dx_native_probe-1.4.dist-info/licenses/msvc_vcruntime");
    assert_eq!(
        std::fs::read(&notice).expect("wheel must ship the runtime notice"),
        std::fs::read(data("DX_MSVC_NOTICE")).expect("declared notice must be readable"),
    );
    let core = core(&members);
    let output = archiver_imports(&members, &core, &["python3*"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let alone: Vec<_> = members
        .iter()
        .filter(|(name, _)| *name == core)
        .cloned()
        .collect();
    let output = archiver_imports(&alone, &core, &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{core} alone, nothing provided:\n{stderr}");
    assert!(stderr.to_ascii_lowercase().contains("python3"), "{stderr}");
    let output = archiver_imports(&alone, &core, &["python3*"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{core} without the shipped runtime:\n{stderr}");
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.to_ascii_lowercase().contains("vcruntime140.dll"),
        "{stderr}"
    );

    let cwd = dx_testing::mkscratch("pypi-native-msvc-cwd").expect("scratch cwd");
    let output = import_from(&target, &cwd);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "dx-native-probe native"
    );
    let moved = target.with_file_name(format!(
        "{}-moved",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("t")
    ));
    std::fs::rename(&target, &moved).expect("rename install dir");
    let output = import_from(&moved, &cwd);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    check_module(&moved, "dx_native_probe", "dx-native-probe native");
}

fn readelf_dynamic(library: &Path) -> String {
    readelf_output("-d", library)
}

fn readelf_output(flag: &str, library: &Path) -> String {
    let output = std::process::Command::new("readelf")
        .arg(flag)
        .arg(library)
        .output()
        .expect("readelf must execute");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn linked_import(target: &Path, code: &str) -> std::process::Output {
    let mut command = std::process::Command::new(data("DX_PYPI_PYTHON"));
    kept_env(&mut command);
    command
        .env_remove("LD_LIBRARY_PATH")
        .env("PYTHONPATH", target)
        .args(["-c", code]);
    no_compilers(&mut command);
    command.output().expect("python must execute")
}

fn pypi_native_linked_case() {
    let call = "from dx_native_probe import _linked; print(_linked.add(2, 3))";
    let house = stage(
        "DX_LINKED_PROG",
        "DX_LINKED_CONFIG",
        "native_probe_linked-wheelhouse",
        "dx-native-probe",
        "native_probe_linked",
    );
    let (url, _log) = serve(house);
    let target = install(&url, "dx_native_probe");
    let package = target.join("dx_native_probe");
    let mut found = Vec::new();
    native_files(&package, &mut found);
    let ext: Vec<&PathBuf> = found
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("_linked") && name.ends_with(".so"))
        })
        .collect();
    assert_eq!(ext.len(), 1, "{found:?}");
    assert!(package.join("libdx_probe_helper.so").is_file(), "{found:?}");
    let notice = target.join("dx_native_probe-1.2.dist-info/licenses/dx_probe_helper_runtime");
    println!("{}", notice.display());
    assert_eq!(
        std::fs::read(&notice).expect("wheel must ship the helper notice"),
        std::fs::read(data("DX_LINKED_NOTICE")).expect("declared notice must be readable"),
    );
    let dynamic = readelf_dynamic(ext[0]);
    println!("{dynamic}");
    assert!(dynamic.contains("libdx_probe_helper.so"), "{dynamic}");
    let runpath: Vec<&str> = dynamic
        .lines()
        .filter(|line| line.contains("RUNPATH") || line.contains("RPATH"))
        .collect();
    assert!(!runpath.is_empty(), "{dynamic}");
    for line in &runpath {
        assert!(line.contains("$ORIGIN"), "{line}");
        assert!(
            !line.contains("bazel-out") && !line.contains("[/"),
            "{line}"
        );
    }

    let moved = target.with_file_name("moved-linked-target");
    std::fs::rename(&target, &moved).expect("move install dir");
    let output = linked_import(&moved, call);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "5");

    let house = stage(
        "DX_NOLIB_PROG",
        "DX_NOLIB_CONFIG",
        "native_probe_nolib-wheelhouse",
        "dx-native-probe",
        "native_probe_nolib",
    );
    let (url, _log) = serve(house);
    let target = install(&url, "dx_native_probe");
    assert!(
        !target
            .join("dx_native_probe/libdx_probe_helper.so")
            .exists()
    );
    let output = linked_import(&target, call);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("libdx_probe_helper.so"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn tar_listing(bytes: &[u8], name: &str) -> Vec<String> {
    let dir = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR must be set"));
    let file = dir.join(name);
    std::fs::write(&file, bytes).expect("write member");
    let run = dx_testing::run(
        &PathBuf::from("tar"),
        &["-tif", file.to_string_lossy().as_ref()],
        &[],
    )
    .expect("tar must execute");
    assert!(
        run.status.success(),
        "cannot list {name}\n{}",
        run.combined()
    );
    let mut names: Vec<String> = run
        .stdout
        .lines()
        .map(|line| line.trim_start_matches("./").to_owned())
        .filter(|line| !line.is_empty() && !line.ends_with('/') && !line.starts_with('.'))
        .collect();
    names.sort();
    names
}

fn deb_files(deb: &[u8]) -> Vec<String> {
    assert!(deb.starts_with(b"!<arch>\n"), "deb is not an ar archive");
    let mut at = 8;
    while at + 60 <= deb.len() {
        let header = &deb[at..at + 60];
        let member = String::from_utf8_lossy(&header[..16]).trim().to_owned();
        let size: usize = String::from_utf8_lossy(&header[48..58])
            .trim()
            .parse()
            .expect("ar member size");
        let body = &deb[at + 60..at + 60 + size];
        if member.starts_with("data.tar") {
            return tar_listing(body, &member);
        }
        at += 60 + size + size % 2;
    }
    panic!("deb has no data.tar member");
}

fn linux_package_case() {
    github_case();
    let want: Vec<String> = {
        let mut files: Vec<String> = std::env::var("DX_PKG_FILES")
            .expect("DX_PKG_FILES must list the installed files")
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        files.sort();
        files
    };
    let deb = std::fs::read(data("DX_PKG_DEB")).expect("read deb");
    assert_eq!(deb_files(&deb), want, "deb contents");

    let apk = std::fs::read(data("DX_PKG_APK")).expect("read apk");
    assert_eq!(tar_listing(&apk, "demo.apk.tar"), want, "apk contents");

    let rpm = std::fs::read(data("DX_PKG_RPM")).expect("read rpm");
    assert!(rpm.starts_with(&[0xed, 0xab, 0xee, 0xdb]), "rpm lead magic");
    for path in &want {
        let base = path.rsplit('/').next().unwrap_or(path);
        assert!(
            rpm.windows(base.len())
                .any(|window| window == base.as_bytes()),
            "rpm does not list {base}"
        );
    }
}

#[test]
fn deploy_verify() {
    let mode = std::env::var("DX_VERIFY_MODE").expect("DX_VERIFY_MODE must select the harness");
    match mode.as_str() {
        "archive" => archive_case(),
        "cloudflare" => cloudflare_case(),
        "github" => github_case(),
        "helm" => helm_case(),
        "homebrew" => homebrew_case(),
        "linux_package" => linux_package_case(),
        "oci" => oci_case(),
        "pypi" => pypi_case(),
        "pypi_native" => pypi_native_case(),
        "pypi_native_linked" => pypi_native_linked_case(),
        "pypi_native_msvc" => pypi_native_msvc_case(),
        "resources" => resources_case(),
        "rubygems" => rubygems_case(),
        other => panic!("unknown DX_VERIFY_MODE: {other}"),
    }
}
