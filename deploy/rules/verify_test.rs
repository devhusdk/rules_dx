use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

fn github_case() {
    let prog = data("DX_GITHUB_PROG");
    let want_tag = std::env::var("DX_GITHUB_TAG").expect("DX_GITHUB_TAG must name the tag");
    let assets = std::env::var("DX_GITHUB_ASSETS").expect("DX_GITHUB_ASSETS must list assets");

    let run = run_deploy(&prog, "DX_GITHUB_CONFIG", &[], &[]).expect("github launcher must execute");
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
    assert!(
        out.contains(&format!("gh release create {want_tag} ")),
        "github would-run command missing 'gh release create {want_tag}'\n{out}"
    );
    assert!(
        out.lines()
            .any(|line| line.ends_with("--draft --verify-tag")),
        "github would-run command missing draft-only flags '--draft --verify-tag'\n{out}"
    );

    let bare = dx_testing::run(&prog, &[], &[])
        .expect("unconfigured run must execute");
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
        ("CLOUDFLARE_PAGES_LIVE", "1"),
        ("CLOUDFLARE_PAGES_APPROVED", "1"),
        ("CLOUDFLARE_API_TOKEN", token),
        ("AMBIENT_SECRET", "ambient"),
        ("DX_PROFILE", "release"),
    ];

    let preview =
        run_deploy(&prog, "DX_CLOUDFLARE_CONFIG", &[], &env).expect("preview deploy must execute");
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

    let live = run_deploy(
        &production,
        "DX_CLOUDFLARE_PRODUCTION_CONFIG",
        &[],
        &[
            ("CLOUDFLARE_PAGES_LIVE", "1"),
            ("CLOUDFLARE_PAGES_APPROVED", "1"),
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
            ("CLOUDFLARE_PAGES_LIVE", "1"),
            ("CLOUDFLARE_PAGES_APPROVED", "1"),
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
            ("CLOUDFLARE_PAGES_LIVE", "1"),
            ("CLOUDFLARE_PAGES_APPROVED", "1"),
            ("CLOUDFLARE_API_TOKEN", ""),
        ],
    )
    .expect("missing token run must execute");
    assert!(!missing.status.success());
    assert!(
        missing.stderr.contains("CLOUDFLARE_API_TOKEN is not set"),
        "{}",
        missing.stderr
    );

    let denied = run_deploy(
        &prog,
        "DX_CLOUDFLARE_CONFIG",
        &[],
        &[
            ("CLOUDFLARE_PAGES_LIVE", "1"),
            ("CLOUDFLARE_PAGES_APPROVED", "1"),
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
        &[("CLOUDFLARE_API_TOKEN", token), ("DX_PROFILE", "release")],
    )
    .expect("check run must execute");
    let out = checked.combined();
    assert!(checked.status.success(), "check run failed\n{out}");
    assert!(
        checked.stdout.contains(
            "checked dx-demo (branch feature/demo, preview, profile: release); nothing uploaded"
        ),
        "{out}"
    );
    assert!(!out.contains("deployment:"), "{out}");
    assert!(!out.contains(token), "token leaked\n{out}");

    let unapproved = run_deploy(
        &prog,
        "DX_CLOUDFLARE_CONFIG",
        &[],
        &[
            ("CLOUDFLARE_PAGES_LIVE", "1"),
            ("CLOUDFLARE_API_TOKEN", token),
        ],
    )
    .expect("unapproved run must execute");
    assert!(!unapproved.status.success(), "{}", unapproved.combined());
    assert!(
        unapproved.stderr.contains("CLOUDFLARE_PAGES_APPROVED=1"),
        "{}",
        unapproved.stderr
    );
    assert!(unapproved.stdout.is_empty(), "{}", unapproved.combined());

    for removed in [
        run_deploy(&prog, "DX_CLOUDFLARE_CONFIG", &["--dry-run"], &env)
            .expect("removed flag must execute"),
        run_deploy(
            &prog,
            "DX_CLOUDFLARE_CONFIG",
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

type RequestLog = Arc<Mutex<Vec<String>>>;

fn serve(root: PathBuf) -> (String, RequestLog) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("local addr").port();
    let log: RequestLog = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let root = root.clone();
            let seen = Arc::clone(&seen);
            std::thread::spawn(move || answer(stream, &root, &seen));
        }
    });
    (format!("http://127.0.0.1:{port}"), log)
}

fn answer(mut stream: std::net::TcpStream, root: &Path, seen: &RequestLog) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let mut words = text.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let target = words.next().unwrap_or_default().to_owned();
    if let Ok(mut log) = seen.lock() {
        log.push(target.clone());
    }
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut file = root.to_path_buf();
    for segment in &segments {
        file.push(segment);
    }
    if path.ends_with('/') || file.is_dir() {
        file.push("index.html");
    }
    let found = !segments.contains(&"..") && file.is_file();
    let (status, kind, body) = if found {
        let kind = if file.extension().is_some_and(|ext| ext == "html") {
            "text/html"
        } else {
            "application/octet-stream"
        };
        ("200 OK", kind, std::fs::read(&file).unwrap_or_default())
    } else {
        ("404 Not Found", "text/plain", Vec::new())
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {kind}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes());
    if method != "HEAD" {
        let _ = stream.write_all(&body);
    }
}

fn stage(var: &str, house_name: &str, target_name: &str) -> PathBuf {
    let prog = data(var);
    let outdir = dx_testing::mkscratch("pypi-out").expect("scratch outdir");
    let run = dx_testing::run(&prog, &[outdir.to_string_lossy().as_ref()], &[])
        .expect("pypi_deploy program must execute");
    assert!(run.status.success(), "{}", run.combined());
    let house = outdir.join(house_name);
    let index = house.join("simple-index");
    assert!(
        index.join("dx-pypi-probe").join("index.html").is_file(),
        "{}",
        run.combined()
    );
    assert!(!index.join(target_name).exists(), "{}", house.display());
    house
}

fn kept_env(command: &mut std::process::Command) {
    command.env_clear();
    for key in ["PATH", "SYSTEMROOT", "TEMP", "TMP", "HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}

fn install(url: &str, spelling: &str) {
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
    assert!(
        output.status.success(),
        "uv pip install {spelling} failed\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut command = std::process::Command::new(&python);
    kept_env(&mut command);
    command
        .env("PYTHONPATH", &target)
        .args(["-m", "dx_pypi_probe"]);
    let output = command.output().expect("python must execute");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "dx-pypi-probe ok",
        "{spelling}"
    );
}

fn requests(log: &RequestLog) -> Vec<String> {
    log.lock().map(|log| log.clone()).unwrap_or_default()
}

fn pypi_case() {
    let wheel = "dx_pypi_probe-1.0-py3-none-any.whl";
    let house = stage("DX_PYPI_PROG", "dx_pypi_probe-wheelhouse", "dx_pypi_probe");
    let (url, log) = serve(house.clone());
    for spelling in [
        "Dx_Pypi.Probe",
        "dx_pypi_probe",
        "DX.PYPI.PROBE",
        "dx--pypi__probe",
        "dx-pypi-probe",
    ] {
        install(&url, spelling);
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
        "pypi_probe_release-wheelhouse",
        "pypi_probe_release",
    );
    assert!(house.join(legacy).is_file(), "{}", house.display());
    let (url, log) = serve(house);
    install(&url, "Dx_Pypi.Probe");
    let seen = requests(&log);
    assert!(
        seen.iter()
            .any(|path| path.ends_with(&format!("/{legacy}"))),
        "{seen:?}"
    );

    let prog = data("DX_PYPI_MISMATCH_PROG");
    let outdir = dx_testing::mkscratch("pypi-mismatch").expect("scratch outdir");
    let run = dx_testing::run(&prog, &[outdir.to_string_lossy().as_ref()], &[])
        .expect("pypi_deploy program must execute");
    assert!(!run.status.success(), "{}", run.combined());
    assert!(
        run.stderr.contains(
            "pypi_deploy: wheel dx_pypi_probe-1.0-py3-none-any.whl is dx-pypi-probe 1.0 but sdist dx_other-1.0.tar.gz is dx-other 1.0"
        ),
        "{}",
        run.stderr
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
        "pypi" => pypi_case(),
        "resources" => resources_case(),
        other => panic!("unknown DX_VERIFY_MODE: {other}"),
    }
}
