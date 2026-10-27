use std::path::{Path, PathBuf};
use std::process::Output;

use dx_testing::{mkscratch, resolve_runfiles, rlocation, rlocation_key};

struct Input {
    key: String,
    path: PathBuf,
}

fn input(name: &str) -> Input {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    Input {
        key: rlocation_key(&rel),
        path: resolve_runfiles(&rel),
    }
}

fn program(name: &str) -> PathBuf {
    input(name).path
}

fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn scratch(name: &str) -> PathBuf {
    mkscratch(name).expect("scratch dir")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("named file")
        .to_string_lossy()
        .into_owned()
}

fn plant(root: &Path, key: &str, body: &[u8]) -> PathBuf {
    let path = root.join(key);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, body).expect("write");
    path
}

fn config_for(program_var: &str) -> PathBuf {
    let config_var = program_var.replace("_PROGRAM", "_CONFIG");
    let config = program(&config_var);
    std::path::absolute(&config).expect("absolute config path")
}

fn launch(program_var: &str, out: &Path, envs: &[(&str, &Path)]) -> Output {
    let mut command = std::process::Command::new(program(program_var));
    command.arg(out);
    command.env("DX_DEPLOY_CONFIG", config_for(program_var));
    for name in ["RUNFILES_MANIFEST_FILE", "RUNFILES_DIR", "TEST_SRCDIR"] {
        command.env_remove(name);
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output().expect("launcher runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn released(out: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(out)
        .expect("list outdir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn archive_inputs() -> [Input; 3] {
    [input("DX_APP"), input("DX_TARBALL"), input("DX_CHECKSUM")]
}

fn assert_released_the_real_archive(out: &Path) {
    for real in [input("DX_TARBALL"), input("DX_CHECKSUM")] {
        let copy = out.join(file_name(&real.path));
        assert_eq!(
            bytes(&copy),
            bytes(&real.path),
            "{} differs",
            copy.display()
        );
    }
}

fn manifest_line(key: &str, value: &Path) -> String {
    let value = value.to_string_lossy();
    if value.contains(['\n', '\\']) && !cfg!(windows) {
        let escaped = value.replace('\\', "\\b").replace('\n', "\\n");
        return format!(" {key} {escaped}\n");
    }
    format!("{key} {value}\n")
}

fn write_manifest(dir: &Path, entries: &[(&str, &Path)]) -> PathBuf {
    let text: String = entries
        .iter()
        .map(|(key, value)| manifest_line(key, value))
        .collect();
    let manifest = dir.join("MANIFEST");
    std::fs::write(&manifest, text).expect("write manifest");
    manifest
}

#[test]
fn an_archive_launcher_releases_what_its_own_runfiles_name() {
    let decoy = scratch("decoy");
    for real in archive_inputs() {
        plant(&decoy, &real.key, b"decoy");
    }
    let out = scratch("own");
    let config = config_for("DX_ARCHIVE_PROGRAM");
    let run = dx_testing::run(
        &program("DX_ARCHIVE_PROGRAM"),
        &[out.to_string_lossy().as_ref()],
        &[
            ("DX_DEPLOY_CONFIG", config.to_string_lossy().as_ref()),
            ("RUNFILES_DIR", decoy.to_string_lossy().as_ref()),
            ("TEST_SRCDIR", decoy.to_string_lossy().as_ref()),
        ],
    )
    .expect("launcher runs");
    assert!(run.status.success(), "{}", run.combined());
    assert_released_the_real_archive(&out);
}

#[test]
fn an_archive_launcher_reads_the_tree_it_is_handed() {
    let tree = scratch("tree");
    for real in archive_inputs() {
        plant(&tree, &real.key, &bytes(&real.path));
    }
    let out = scratch("tree-out");
    let run = launch("DX_ARCHIVE_PROGRAM", &out, &[("RUNFILES_DIR", &tree)]);
    assert!(run.status.success(), "{}", stderr(&run));
    assert_released_the_real_archive(&out);

    let decoy = scratch("decoy-tree");
    for real in archive_inputs() {
        plant(&decoy, &real.key, b"decoy");
    }
    let tarball = decoy.join(input("DX_TARBALL").key);
    let out = scratch("decoy-out");
    let run = launch("DX_ARCHIVE_PROGRAM", &out, &[("RUNFILES_DIR", &decoy)]);
    assert!(!run.status.success(), "a decoy tree must not release");
    let text = stderr(&run);
    assert!(text.contains("checksum mismatch"), "{text}");
    assert!(
        text.contains(&tarball.display().to_string()),
        "the error names the file it read: {text}"
    );
    assert!(released(&out).is_empty(), "{:?}", released(&out));
}

#[test]
fn an_archive_launcher_reads_escaped_manifest_entries() {
    let root = scratch("escaped");
    let dir = if cfg!(windows) {
        root.join("dir with space")
    } else {
        root.join("dir with space\\and\nnewline")
    };
    std::fs::create_dir_all(&dir).expect("escaped dir");
    let mut entries = Vec::new();
    for real in archive_inputs() {
        let copy = dir.join(file_name(&real.path));
        std::fs::copy(&real.path, &copy).expect("copy input");
        entries.push((real.key, copy));
    }
    let borrowed: Vec<(&str, &Path)> = entries
        .iter()
        .map(|(key, path)| (key.as_str(), path.as_path()))
        .collect();
    let manifest = write_manifest(&root, &borrowed);
    let out = scratch("escaped-out");
    let run = launch(
        "DX_ARCHIVE_PROGRAM",
        &out,
        &[("RUNFILES_MANIFEST_FILE", &manifest)],
    );
    assert!(run.status.success(), "{}", stderr(&run));
    assert_released_the_real_archive(&out);
}

#[test]
fn an_archive_launcher_names_a_missing_key_and_releases_nothing() {
    let [app, tarball, checksum] = archive_inputs();
    let root = scratch("missing");
    let manifest = write_manifest(
        &root,
        &[
            (app.key.as_str(), app.path.as_path()),
            (tarball.key.as_str(), tarball.path.as_path()),
        ],
    );
    let out = scratch("missing-out");
    let run = launch(
        "DX_ARCHIVE_PROGRAM",
        &out,
        &[("RUNFILES_MANIFEST_FILE", &manifest)],
    );
    assert_eq!(run.status.code(), Some(1));
    let text = stderr(&run);
    assert!(text.contains(&checksum.key), "{text}");
    assert!(released(&out).is_empty(), "{:?}", released(&out));
}

fn fake_gh(dir: &Path, marker: &Path) {
    std::fs::create_dir_all(dir).expect("fake bin");
    let marker = marker.display();
    std::fs::write(
        dir.join("gh"),
        format!("#!/bin/sh\necho ran > '{marker}'\n"),
    )
    .expect("fake gh");
    std::fs::write(dir.join("gh.cmd"), format!("@echo ran> \"{marker}\"\r\n")).expect("fake gh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir.join("gh"), std::fs::Permissions::from_mode(0o755))
            .expect("chmod");
    }
}

#[test]
fn a_github_launcher_stages_an_external_input_without_publishing() {
    let root = scratch("github");
    let marker = root.join("gh-ran");
    let bin = root.join("bin");
    fake_gh(&bin, &marker);
    let out = root.join("out");
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .expect("PATH");
    let config = config_for("DX_GITHUB_PROGRAM");
    let run = dx_testing::run(
        &program("DX_GITHUB_PROGRAM"),
        &[out.to_string_lossy().as_ref()],
        &[
            ("DX_DEPLOY_CONFIG", config.to_string_lossy().as_ref()),
            ("GH_RELEASE_DRY_RUN", "1"),
            ("PATH", path.to_string_lossy().as_ref()),
        ],
    )
    .expect("launcher runs");
    assert!(run.status.success(), "{}", run.combined());
    let staging = out.join("identity_github-release");
    for real in [input("DX_TARBALL"), input("DX_NOTICE")] {
        let copy = staging.join(file_name(&real.path));
        assert_eq!(
            bytes(&copy),
            bytes(&real.path),
            "{} differs",
            copy.display()
        );
    }
    assert!(!marker.exists(), "a dry run must not run gh");
}

#[test]
fn a_github_launcher_names_a_missing_external_key() {
    let tarball = input("DX_TARBALL");
    let notice = input("DX_NOTICE");
    let root = scratch("github-missing");
    let manifest = write_manifest(&root, &[(tarball.key.as_str(), tarball.path.as_path())]);
    let out = root.join("out");
    let run = launch(
        "DX_GITHUB_PROGRAM",
        &out,
        &[("RUNFILES_MANIFEST_FILE", &manifest)],
    );
    assert_eq!(run.status.code(), Some(1));
    let text = stderr(&run);
    assert!(text.contains(&notice.key), "{text}");
    assert!(!out.join("identity_github-release").exists());
}

#[test]
fn an_apparent_repository_name_resolves_to_the_canonical_input() {
    let notice = input("DX_NOTICE");
    let (canonical, rest) = notice.key.split_once('/').expect("repository key");
    assert_ne!(canonical, "consumer_archive_repo", "{}", notice.key);
    let apparent = format!("consumer_archive_repo/{rest}");
    let mapped = rlocation(&apparent).expect("the parent maps the apparent name");
    assert_eq!(bytes(&mapped), bytes(&notice.path));
    let child = dx_path::Resolver::for_binary(&program("DX_GITHUB_PROGRAM"))
        .expect("the launcher carries its own runfiles");
    let own = child
        .lookup(&apparent)
        .expect("the child maps the apparent name");
    assert_eq!(bytes(&own), bytes(&notice.path));
    assert!(child.lookup("other_repo/NOTICE.txt").is_err());
}
