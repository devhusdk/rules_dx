use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;

const RUN_ENV: [(&str, &str); 3] = [
    ("APPIMAGE_EXTRACT_AND_RUN", "1"),
    ("LD_LIBRARY_PATH", ""),
    ("PATH", ""),
];

fn env_path(variable: &str) -> PathBuf {
    let rel = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set"));
    dx_testing::resolve_runfiles(&rel)
}

fn built(suffix: &str) -> PathBuf {
    from("DX_APPIMAGE_FILES", suffix)
}

fn from(variable: &str, suffix: &str) -> PathBuf {
    let files = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set"));
    let rel = files
        .split_whitespace()
        .find(|path| path.ends_with(suffix))
        .unwrap_or_else(|| panic!("no built file ending {suffix}"));
    dx_testing::resolve_runfiles(rel)
}

static SPAWN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn spawn_guard() -> std::sync::MutexGuard<'static, ()> {
    SPAWN_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn relocate(source: &Path, dir: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let _guard = spawn_guard();
    let scratch = dx_testing::mkscratch("appimage").expect("scratch");
    let moved = scratch.join(dir);
    std::fs::create_dir_all(&moved).expect("create directory");
    let target = moved.join("moved.AppImage");
    std::fs::copy(source, &target).expect("copy image");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    target
}

fn run_image(image: &Path, args: &[&str]) -> dx_testing::Run {
    let tmp = image.parent().expect("parent").join("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let tmp = tmp.to_str().expect("utf-8");
    let mut envs = RUN_ENV.to_vec();
    envs.push(("TMPDIR", tmp));
    envs.push(("HOME", tmp));
    let _guard = spawn_guard();
    dx_testing::run(image, args, &envs).expect("spawn image")
}

fn extract(image: &Path) -> PathBuf {
    let dir = image.parent().expect("parent").join("extracted");
    std::fs::create_dir_all(&dir).expect("create extract dir");
    let _guard = spawn_guard();
    let output = std::process::Command::new(image)
        .arg("--appimage-extract")
        .current_dir(&dir)
        .env_clear()
        .output()
        .expect("extract");
    assert!(
        output.status.success(),
        "extract failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    dir.join("squashfs-root")
}

fn identity() -> Value {
    dx_testing::read_json(&built(".identity.json")).expect("identity json")
}

fn highest_glibc(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut best: Vec<u64> = Vec::new();
    for token in text.split("GLIBC_").skip(1) {
        let version: String = token
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let parts: Vec<u64> = version
            .trim_end_matches('.')
            .split('.')
            .filter_map(|part| part.parse().ok())
            .collect();
        if !parts.is_empty() && parts > best {
            best = parts;
        }
    }
    best.iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

#[test]
fn relocated_image_runs_without_fuse_or_host_libraries() {
    let image = relocate(&built(".AppImage"), "reloc dir");
    let run = run_image(&image, &[]);
    assert!(run.status.success(), "run failed: {}", run.combined());
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines[0], "hello from the packaged resource");
    assert_eq!(lines[1], "greeting from libgreet");
    let appdir = lines[2].strip_prefix("appdir=").expect("appdir line");
    let tmp = image.parent().expect("parent").join("tmp");
    assert!(
        Path::new(appdir).starts_with(&tmp) && appdir.contains("appimage_extracted_"),
        "image did not extract and run from TMPDIR: {appdir}"
    );
}

#[test]
fn contents_match_the_machine_readable_identity() {
    let image = relocate(&built(".AppImage"), "inspect");
    let root = extract(&image);
    let identity = identity();
    let files = identity["files"].as_object().expect("files object");
    let expected = [
        "dxhello.desktop",
        "dxhello.png",
        "usr/bin/dxhello",
        "usr/lib/libgreet_shared.so",
        "usr/share/dxhello/fixtures/greeting.txt",
        "usr/share/licenses/dxhello/NOTICE.txt",
    ];
    let mut names: Vec<&str> = files.keys().map(String::as_str).collect();
    names.sort_unstable();
    let mut want = expected.to_vec();
    want.sort_unstable();
    assert_eq!(names, want);
    for (path, digest) in files {
        let actual = dx_testing::sha256_file(&root.join(path)).expect("hash extracted file");
        assert_eq!(digest.as_str(), Some(actual.as_str()), "{path}");
    }
    assert_eq!(
        std::fs::read_link(root.join("AppRun")).expect("AppRun link"),
        Path::new("usr/bin/dxhello")
    );
    let desktop = std::fs::read_to_string(root.join("dxhello.desktop")).expect("desktop");
    assert!(desktop.lines().any(|line| line == "Exec=dxhello"));
    assert_eq!(identity["name"], "dxhello");
    assert_eq!(identity["version"], "1.2.3");
    assert_eq!(identity["arch"], "x86_64");
    assert_eq!(identity["appimagetool"], "1.9.1");
    assert_eq!(
        identity["image_sha256"].as_str(),
        Some(
            dx_testing::sha256_file(&built(".AppImage"))
                .expect("hash image")
                .as_str()
        )
    );
}

#[test]
fn loader_and_libc_identity_come_from_the_packaged_bytes() {
    let image = relocate(&built(".AppImage"), "libc");
    let root = extract(&image);
    let bytes = std::fs::read(root.join("usr/bin/dxhello")).expect("packaged binary");
    let identity = identity();
    let floor = highest_glibc(&bytes);
    assert!(!floor.is_empty(), "binary names no glibc version");
    assert_eq!(identity["min_glibc"].as_str(), Some(floor.as_str()));
    let interpreter = identity["interpreter"].as_str().expect("interpreter");
    assert!(
        interpreter.contains("ld-linux"),
        "unexpected interpreter {interpreter}"
    );
    assert!(String::from_utf8_lossy(&bytes).contains(interpreter));
    assert!(String::from_utf8_lossy(&bytes).contains("$ORIGIN/../lib"));
}

#[test]
fn image_starts_with_the_pinned_runtime() {
    let runtime = std::fs::read(env_path("DX_APPIMAGE_RUNTIME")).expect("runtime");
    let image = std::fs::read(built(".AppImage")).expect("image");
    assert!(
        image.len() >= runtime.len(),
        "image is shorter than the runtime: {} < {}",
        image.len(),
        runtime.len()
    );
    let differing: Vec<usize> = (0..runtime.len())
        .filter(|&index| image[index] != runtime[index])
        .collect();
    if let (Some(&first), Some(&last)) = (differing.first(), differing.last()) {
        assert!(
            last - first < 16,
            "runtime bytes changed outside the reserved digest field: {first}..{last}"
        );
    }
    let identity = identity();
    assert_eq!(
        identity["runtime"]["sha256"].as_str(),
        Some(
            dx_testing::sha256_file(&env_path("DX_APPIMAGE_RUNTIME"))
                .expect("hash runtime")
                .as_str()
        )
    );
}

#[test]
fn missing_resource_fails_clearly() {
    let image = relocate(&from("DX_APPIMAGE_NORESOURCE", ".AppImage"), "noresource");
    let run = run_image(&image, &[]);
    assert_eq!(run.status.code(), Some(3), "{}", run.combined());
    assert!(
        run.stderr.contains("dxhello: missing resource"),
        "{}",
        run.stderr
    );
}

#[test]
fn missing_library_fails_in_the_loader() {
    let image = relocate(&from("DX_APPIMAGE_NOLIB", ".AppImage"), "nolib");
    let run = run_image(&image, &[]);
    assert!(!run.status.success());
    assert!(run.stderr.contains("libgreet_shared.so"), "{}", run.stderr);
}

#[test]
fn truncated_and_corrupted_packages_fail() {
    let source = built(".AppImage");
    let runtime_len = std::fs::metadata(env_path("DX_APPIMAGE_RUNTIME"))
        .expect("runtime metadata")
        .len() as usize;
    let bytes = std::fs::read(&source).expect("image");

    let truncated = relocate(&source, "truncated");
    std::fs::write(&truncated, &bytes[..bytes.len() / 2]).expect("truncate");
    let run = run_image(&truncated, &[]);
    assert!(!run.status.success(), "truncated image ran: {}", run.stdout);

    let corrupted = relocate(&source, "corrupted");
    let mut damaged = bytes.clone();
    for byte in &mut damaged[runtime_len..runtime_len + 4096] {
        *byte = 0xff;
    }
    std::fs::write(&corrupted, &damaged).expect("corrupt");
    let run = run_image(&corrupted, &[]);
    assert!(!run.status.success(), "corrupted image ran: {}", run.stdout);
}
