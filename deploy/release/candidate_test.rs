use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use dx_release_tools::candidate::{
    build_source_archive, candidate_module_file, read_git_state, source_archive_error,
    stage_local_registry, write_release_source, Candidate,
};
use dx_release_tools::{
    bcr_candidate_error, bcr_integrity_of_digest, release_remote_error, release_run, RELEASE_REMOTE,
};

const VERSION: &str = "1.2.3-rc1";
const TAG: &str = "v1.2.3-rc1";

const MODULE: &str = r#"module(
    name = "rules_dx",
    version = "0.0.0",
    compatibility_level = 1,
)

bazel_dep(name = "rules_cc", version = "0.1.1")
single_version_override(
    module_name = "rules_cc",
    patch_strip = 1,
    patches = ["//patches:x.patch"],
)

ext = use_extension("//lib:defs.bzl", "ext")
"#;

fn git_home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let home = tempfile::Builder::new()
            .prefix("dx-candidate-home-")
            .tempdir()
            .expect("home")
            .keep();
        std::fs::write(home.join(".gitconfig"), "").expect("gitconfig");
        std::env::set_var("HOME", &home);
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        std::env::set_var("GIT_CONFIG_GLOBAL", home.join(".gitconfig"));
        std::env::set_var("GIT_CEILING_DIRECTORIES", std::env::temp_dir());
        home
    })
}

fn git(dir: &Path, args: &[&str]) -> String {
    git_home();
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Candidate Test",
            "-c",
            "user.email=candidate@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, bytes).expect("write");
}

struct Fixture {
    dir: tempfile::TempDir,
    repo: PathBuf,
    canonical: PathBuf,
    foreign: PathBuf,
    head: String,
}

impl Fixture {
    fn new() -> Self {
        git_home();
        let dir = tempfile::Builder::new()
            .prefix("dx-candidate-")
            .tempdir()
            .expect("scratch");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        git(&repo, &["init", "-q"]);
        write(&repo.join("MODULE.bazel"), MODULE.as_bytes());
        write(
            &repo.join("patches/BUILD.bazel"),
            b"exports_files([\"x.patch\"])\n",
        );
        write(
            &repo.join("patches/x.patch"),
            b"--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n",
        );
        write(&repo.join(".gitattributes"), b"*.cmd text eol=crlf\n");
        write(&repo.join("a.cmd"), b"@echo off\nrem candidate\n");
        write(&repo.join("b=c.txt"), b"equals\n");
        write(&repo.join("tool.sh"), b"#!/bin/sh\necho tool\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(repo.join("tool.sh"), std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        write(
            &repo.join("lib/BUILD.bazel"),
            b"exports_files([\"defs.bzl\"], visibility = [\"//visibility:public\"])\n",
        );
        write(
            &repo.join("lib/defs.bzl"),
            b"def hello():\n    return \"hello\"\n",
        );
        git(&repo, &["add", "-A"]);
        git(&repo, &["update-index", "--chmod=+x", "tool.sh"]);
        git(&repo, &["commit", "-q", "-m", "candidate"]);
        let head = git(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
        let canonical = dir.path().join("canonical.git");
        let foreign = dir.path().join("foreign.git");
        for remote in [&canonical, &foreign] {
            git(
                dir.path(),
                &["init", "-q", "--bare", remote.to_str().expect("utf8")],
            );
        }
        Self {
            dir,
            repo,
            canonical,
            foreign,
            head,
        }
    }

    fn candidate(&self) -> Candidate {
        Candidate::new(VERSION, &self.head).expect("candidate")
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn refs(&self) -> (String, String) {
        (
            git(
                &self.repo,
                &["ls-remote", self.canonical.to_str().expect("utf8")],
            ),
            git(
                &self.repo,
                &["ls-remote", self.foreign.to_str().expect("utf8")],
            ),
        )
    }

    fn ls_remote(&self, remote: &Path) -> Result<String, String> {
        let [reference, peeled] = self.candidate().ls_remote_patterns();
        let out = Command::new("git")
            .args([
                "ls-remote",
                "--tags",
                remote.to_str().expect("utf8"),
                &reference,
                &peeled,
            ])
            .current_dir(&self.repo)
            .output()
            .expect("spawn git");
        if out.status.success() {
            Ok(String::from_utf8(out.stdout).expect("utf8"))
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }

    fn push_tag(&self, remote: &Path) {
        git(
            &self.repo,
            &[
                "push",
                "-q",
                remote.to_str().expect("utf8"),
                &format!("refs/tags/{TAG}"),
            ],
        );
    }

    fn advance(&self) -> String {
        write(&self.repo.join("later.txt"), b"later\n");
        git(&self.repo, &["add", "later.txt"]);
        git(&self.repo, &["commit", "-q", "-m", "later"]);
        git(&self.repo, &["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn archive(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        build_source_archive(&self.repo, &self.candidate(), &path).expect("archive");
        path
    }
}

fn members(archive: &Path) -> Vec<(String, Vec<u8>, u32)> {
    dx_deploy_tools::archive_listing(archive)
        .expect("listing")
        .into_iter()
        .map(|(name, mode, _)| {
            let data = dx_deploy_tools::archive_read_member(archive, &name).expect("member");
            (name, data, mode)
        })
        .collect()
}

fn repack(members: &[(String, Vec<u8>, u32)], dst: &Path) {
    let refs: Vec<(&str, &[u8], u32)> = members
        .iter()
        .map(|(name, data, mode)| (name.as_str(), data.as_slice(), *mode))
        .collect();
    dx_deploy_tools::archive_tree(&refs, dst).expect("repack");
}

fn live_run(fixture: &Fixture, commit: &str) -> Result<String, String> {
    release_run(TAG, Some(commit), "1", false, || {
        read_git_state(Some(&fixture.repo), TAG)
    })
}

#[test]
fn archive_builds_identical_bytes_and_digest() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let first = fixture.path("first.tar.gz");
    let second = fixture.path("second.tar.gz");
    let one = build_source_archive(&fixture.repo, &candidate, &first).expect("first");
    let two = build_source_archive(&fixture.repo, &candidate, &second).expect("second");
    assert_eq!(one, two);
    assert_eq!(
        std::fs::read(&first).expect("first"),
        std::fs::read(&second).expect("second")
    );
    assert_eq!(
        one.digest,
        dx_digest::sha256_file_hex(&first).expect("digest")
    );
    assert_eq!(
        one.integrity,
        bcr_integrity_of_digest(&one.digest).expect("integrity")
    );
    source_archive_error(&first, &fixture.repo, &candidate).expect("verifies");
}

#[test]
fn archive_members_equal_the_commit_tree() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.repo.join(".gitattributes")).expect("attributes");
    let archive = fixture.archive("tree.tar.gz");
    let tree = git(
        &fixture.repo,
        &["ls-tree", "-r", "--name-only", &fixture.head],
    );
    let mut want: Vec<String> = tree
        .lines()
        .map(|path| format!("rules_dx-{VERSION}/{path}"))
        .collect();
    want.sort();
    let listing = dx_deploy_tools::archive_listing(&archive).expect("listing");
    let names: Vec<String> = listing.iter().map(|entry| entry.0.clone()).collect();
    assert_eq!(names, want);
    let mode_of = |name: &str| {
        listing
            .iter()
            .find(|entry| entry.0 == format!("rules_dx-{VERSION}/{name}"))
            .map(|entry| entry.1)
            .expect(name)
    };
    assert_eq!(mode_of("tool.sh"), 0o755);
    assert_eq!(mode_of("MODULE.bazel"), 0o644);
    assert_eq!(mode_of("b=c.txt"), 0o644);
    let cmd = dx_deploy_tools::archive_read_member(&archive, &format!("rules_dx-{VERSION}/a.cmd"))
        .expect("cmd");
    assert_eq!(cmd, b"@echo off\r\nrem candidate\r\n");
    let module =
        dx_deploy_tools::archive_read_member(&archive, &format!("rules_dx-{VERSION}/MODULE.bazel"))
            .expect("module");
    assert_eq!(module, MODULE.as_bytes());
}

#[test]
fn archive_rejects_a_dropped_member() {
    let fixture = Fixture::new();
    let before = fixture.refs();
    let archive = fixture.archive("good.tar.gz");
    let mut kept = members(&archive);
    let dropped = format!("rules_dx-{VERSION}/patches/x.patch");
    kept.retain(|member| member.0 != dropped);
    let bad = fixture.path("dropped.tar.gz");
    repack(&kept, &bad);
    assert_eq!(
        source_archive_error(&bad, &fixture.repo, &fixture.candidate()),
        Err(format!("candidate: archive drops member {dropped}"))
    );
    assert_eq!(fixture.refs(), before);
}

#[test]
fn archive_rejects_a_changed_blob_and_mode() {
    let fixture = Fixture::new();
    let archive = fixture.archive("good.tar.gz");
    let module = format!("rules_dx-{VERSION}/MODULE.bazel");
    let mut changed = members(&archive);
    for member in &mut changed {
        if member.0 == module {
            member
                .1
                .extend_from_slice(b"bazel_dep(name = \"evil\", version = \"1\")\n");
        }
    }
    let bad = fixture.path("changed.tar.gz");
    repack(&changed, &bad);
    assert_eq!(
        source_archive_error(&bad, &fixture.repo, &fixture.candidate()),
        Err(format!(
            "candidate: archive member {module} differs from commit {}",
            fixture.head
        ))
    );
    let tool = format!("rules_dx-{VERSION}/tool.sh");
    let mut moded = members(&archive);
    for member in &mut moded {
        if member.0 == tool {
            member.2 = 0o644;
        }
    }
    repack(&moded, &bad);
    assert_eq!(
        source_archive_error(&bad, &fixture.repo, &fixture.candidate()),
        Err(format!(
            "candidate: archive member {tool} has mode 644, want 755"
        ))
    );
    let mut extra = members(&archive);
    extra.push((
        format!("rules_dx-{VERSION}/extra.txt"),
        b"x".to_vec(),
        0o644,
    ));
    repack(&extra, &bad);
    assert_eq!(
        source_archive_error(&bad, &fixture.repo, &fixture.candidate()),
        Err(format!(
            "candidate: archive has extra member rules_dx-{VERSION}/extra.txt"
        ))
    );
}

#[test]
fn archive_rejects_a_wrong_strip_prefix() {
    let fixture = Fixture::new();
    let archive = fixture.archive("good.tar.gz");
    let renamed: Vec<(String, Vec<u8>, u32)> = members(&archive)
        .into_iter()
        .map(|(name, data, mode)| {
            (
                name.replacen(&format!("rules_dx-{VERSION}/"), "rules_dx-9.9.9/", 1),
                data,
                mode,
            )
        })
        .collect();
    let bad = fixture.path("prefix.tar.gz");
    repack(&renamed, &bad);
    let error =
        source_archive_error(&bad, &fixture.repo, &fixture.candidate()).expect_err("wrong prefix");
    assert!(
        error.contains("is outside strip prefix rules_dx-1.2.3-rc1/"),
        "{error}"
    );
}

#[test]
fn archive_rejects_a_foreign_sha() {
    let fixture = Fixture::new();
    let other = fixture.path("other");
    std::fs::create_dir_all(&other).expect("other");
    git(&other, &["init", "-q"]);
    write(&other.join("x.txt"), b"x\n");
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-q", "-m", "foreign"]);
    let foreign = git(&other, &["rev-parse", "HEAD"]).trim().to_owned();
    let candidate = Candidate::new(VERSION, &foreign).expect("candidate");
    let dst = fixture.path("foreign.tar.gz");
    let error = build_source_archive(&fixture.repo, &candidate, &dst).expect_err("foreign sha");
    assert!(
        error.starts_with(&format!("candidate: commit {foreign} is not in ")),
        "{error}"
    );
    assert!(!dst.exists());
    let error = build_source_archive(&fixture.path("missing"), &fixture.candidate(), &dst)
        .expect_err("non-repo");
    assert!(error.contains("is not in"), "{error}");
}

#[cfg(unix)]
#[test]
fn archive_rejects_symlinks() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink("MODULE.bazel", fixture.repo.join("link")).expect("symlink");
    git(&fixture.repo, &["add", "link"]);
    git(&fixture.repo, &["commit", "-q", "-m", "link"]);
    let head = git(&fixture.repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let candidate = Candidate::new(VERSION, &head).expect("candidate");
    let error = build_source_archive(&fixture.repo, &candidate, &fixture.path("link.tar.gz"))
        .expect_err("symlink");
    assert_eq!(
        error,
        "candidate: link is a blob with mode 120000; only regular files are archived"
    );
}

#[test]
fn registry_stage_passes_bcr_check_and_keeps_deps() {
    let fixture = Fixture::new();
    let before = fixture.refs();
    let candidate = fixture.candidate();
    let archive = fixture.archive("good.tar.gz");
    let digest = dx_digest::sha256_file_hex(&archive).expect("digest");
    let registry = fixture.path("registry");
    assert_eq!(
        stage_local_registry(&registry, &candidate, &archive).expect("registry"),
        registry
    );
    let version_dir = registry.join("modules/rules_dx").join(VERSION);
    let source = std::fs::read_to_string(version_dir.join("source.json")).expect("source");
    bcr_candidate_error(&source, "rules_dx", VERSION, &digest).expect("bcr check");
    let module = std::fs::read_to_string(version_dir.join("MODULE.bazel")).expect("module");
    assert_eq!(
        module,
        candidate_module_file(MODULE, VERSION).expect("module file")
    );
    assert!(module.contains("version = \"1.2.3-rc1\""), "{module}");
    assert!(module.contains("bazel_dep(name = \"rules_cc\", version = \"0.1.1\")"));
    assert!(module.contains("patches = [\"//patches:x.patch\"]"));
    assert!(module.contains("use_extension(\"//lib:defs.bzl\", \"ext\")"));
    let mirrored = registry.join(format!(
        "mirror/github.com/devhusdk/rules_dx/releases/download/{TAG}/rules_dx-{VERSION}.tar.gz"
    ));
    assert_eq!(
        std::fs::read(&mirrored).expect("mirror"),
        std::fs::read(&archive).expect("archive")
    );
    let config = std::fs::read_to_string(registry.join("bazel_registry.json")).expect("config");
    assert!(
        config.contains(&format!("file://{}/mirror/", registry.display())),
        "{config}"
    );
    let metadata =
        std::fs::read_to_string(registry.join("modules/rules_dx/metadata.json")).expect("meta");
    assert!(metadata.contains("\"1.2.3-rc1\""), "{metadata}");
    assert_eq!(fixture.refs(), before);
}

#[test]
fn registry_source_rejects_wrong_version_prefix_and_integrity() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let archive = fixture.archive("good.tar.gz");
    let digest = dx_digest::sha256_file_hex(&archive).expect("digest");
    let registry = fixture.path("registry");
    stage_local_registry(&registry, &candidate, &archive).expect("registry");
    let source = std::fs::read_to_string(registry.join("modules/rules_dx/1.2.3-rc1/source.json"))
        .expect("source");
    let error = bcr_candidate_error(&source, "rules_dx", "1.2.4", &digest).expect_err("version");
    assert!(error.contains("strip_prefix mismatch"), "{error}");
    let prefix = source.replace("\"rules_dx-1.2.3-rc1\"", "\"rules_dx-1.2.3\"");
    let error = bcr_candidate_error(&prefix, "rules_dx", VERSION, &digest).expect_err("prefix");
    assert!(error.contains("strip_prefix mismatch"), "{error}");
    let error =
        bcr_candidate_error(&source, "rules_dx", VERSION, &"0".repeat(64)).expect_err("integrity");
    assert!(error.contains("integrity mismatch"), "{error}");
    assert!(Candidate::new("0.0.0", &fixture.head).is_err());
    let error = stage_local_registry(Path::new("relative"), &candidate, &archive)
        .expect_err("relative registry");
    assert!(error.contains("must be absolute"), "{error}");
}

#[test]
fn tag_lightweight_is_peeled_through_ls_remote() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["tag", TAG]);
    fixture.push_tag(&fixture.canonical);
    let before = fixture.refs();
    let output = fixture.ls_remote(&fixture.canonical).expect("ls-remote");
    assert_eq!(output, format!("{}\trefs/tags/{TAG}\n", fixture.head));
    assert_eq!(
        release_remote_error(&fixture.candidate(), Ok(output)),
        Ok(fixture.head.clone())
    );
    let state = read_git_state(Some(&fixture.repo), TAG).expect("state");
    assert!(!state.tree_dirty);
    assert_eq!(state.head, fixture.head);
    assert_eq!(state.tag_commit.as_deref(), Some(fixture.head.as_str()));
    let live = live_run(&fixture, &fixture.head).expect("live");
    assert!(live.contains(RELEASE_REMOTE), "{live}");
    assert_eq!(fixture.refs(), before);
}

#[test]
fn tag_annotated_is_peeled_through_ls_remote() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["tag", "-a", "-m", "candidate", TAG]);
    fixture.push_tag(&fixture.canonical);
    let before = fixture.refs();
    let object = git(&fixture.repo, &["rev-parse", TAG]).trim().to_owned();
    assert_ne!(object, fixture.head);
    let output = fixture.ls_remote(&fixture.canonical).expect("ls-remote");
    assert!(
        output.contains(&format!("{object}\trefs/tags/{TAG}\n")),
        "{output}"
    );
    assert!(
        output.contains(&format!("{}\trefs/tags/{TAG}^{{}}\n", fixture.head)),
        "{output}"
    );
    assert_eq!(
        release_remote_error(&fixture.candidate(), Ok(output)),
        Ok(fixture.head.clone())
    );
    let state = read_git_state(Some(&fixture.repo), TAG).expect("state");
    assert_eq!(state.tag_commit.as_deref(), Some(fixture.head.as_str()));
    live_run(&fixture, &fixture.head).expect("live");
    assert_eq!(fixture.refs(), before);
}

#[test]
fn tag_moved_to_another_commit_points_elsewhere() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["tag", "-a", "-m", "old", TAG]);
    fixture.push_tag(&fixture.canonical);
    let newer = fixture.advance();
    let before = fixture.refs();
    let candidate = Candidate::new(VERSION, &newer).expect("candidate");
    let output = fixture.ls_remote(&fixture.canonical).expect("ls-remote");
    assert_eq!(
        release_remote_error(&candidate, Ok(output)),
        Err(format!(
            "release: remote tag {TAG} points at {}, candidate is {newer} (git ls-remote --tags {RELEASE_REMOTE})",
            fixture.head
        ))
    );
    assert_eq!(
        live_run(&fixture, &newer),
        Err(format!(
            "release: local tag {TAG} points at {}, candidate is {newer}",
            fixture.head
        ))
    );
    assert_eq!(fixture.refs(), before);
}

#[test]
fn tag_stale_sha_fails_when_head_moved_on() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["tag", TAG]);
    let newer = fixture.advance();
    let before = fixture.refs();
    assert_eq!(
        live_run(&fixture, &fixture.head),
        Err(format!(
            "release: HEAD is {newer}, candidate is {}; check out the candidate commit",
            fixture.head
        ))
    );
    assert_eq!(fixture.refs(), before);
}

#[test]
fn tag_missing_locally_fails() {
    let fixture = Fixture::new();
    let before = fixture.refs();
    assert_eq!(
        live_run(&fixture, &fixture.head),
        Err(format!(
            "release: local tag {TAG} missing; push it beforehand with owner approval (this program never creates tags)"
        ))
    );
    assert_eq!(fixture.refs(), before);
}

#[test]
fn remote_tag_only_in_the_foreign_remote_fails() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["tag", TAG]);
    fixture.push_tag(&fixture.foreign);
    let before = fixture.refs();
    assert!(before.1.contains(TAG));
    let output = fixture.ls_remote(&fixture.canonical).expect("ls-remote");
    assert_eq!(output, "");
    assert_eq!(
        release_remote_error(&fixture.candidate(), Ok(output)),
        Err(format!(
            "release: remote has no tag {TAG} (git ls-remote --tags {RELEASE_REMOTE}); push it beforehand with owner approval (this program never creates or pushes tags)"
        ))
    );
    assert_eq!(fixture.refs(), before);
}

#[test]
fn remote_nonexistent_path_surfaces_the_git_error() {
    let fixture = Fixture::new();
    let before = fixture.refs();
    let missing = fixture.path("nowhere.git");
    let failed = fixture.ls_remote(&missing).expect_err("missing remote");
    let error =
        release_remote_error(&fixture.candidate(), Err(failed.clone())).expect_err("failed git");
    assert!(
        error.starts_with(&format!(
            "release: cannot verify remote tag {TAG} (git ls-remote --tags {RELEASE_REMOTE} failed: "
        )),
        "{error}"
    );
    assert!(!failed.is_empty() && error.contains(&failed), "{error}");
    assert_eq!(fixture.refs(), before);
}

#[test]
fn tag_dirty_tree_and_non_repo_fail_closed() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["tag", TAG]);
    let before = fixture.refs();
    write(
        &fixture.repo.join("MODULE.bazel"),
        b"module(name = \"x\")\n",
    );
    assert_eq!(
        live_run(&fixture, &fixture.head),
        Err("release: tree is dirty; release requires a clean tree".to_owned())
    );
    let plain = fixture.path("plain");
    std::fs::create_dir_all(&plain).expect("plain");
    let error = release_run(TAG, Some(&fixture.head), "1", false, || {
        read_git_state(Some(&plain), TAG)
    })
    .expect_err("non-repo");
    assert!(
        error.starts_with("release: cannot read git state: git status --porcelain failed: "),
        "{error}"
    );
    assert_eq!(fixture.refs(), before);
}

#[test]
fn release_source_writes_archive_sidecar_and_source_json() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let out = fixture.path("release-source");
    let built = write_release_source(&fixture.repo, &candidate, &out).expect("release source");
    let name = format!("rules_dx-{VERSION}.tar.gz");
    let archive = out.join(&name);
    source_archive_error(&archive, &fixture.repo, &candidate).expect("verifies");
    assert_eq!(
        built.digest,
        dx_digest::sha256_file_hex(&archive).expect("digest")
    );
    assert_eq!(
        std::fs::read_to_string(out.join(format!("{name}.sha256"))).expect("sidecar"),
        format!("{}  {name}\n", built.digest)
    );
    let text = std::fs::read_to_string(out.join("source.json")).expect("source.json");
    bcr_candidate_error(&text, "rules_dx", VERSION, &built.digest).expect("source.json");
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .expect("dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            name.clone(),
            format!("{name}.sha256"),
            "source.json".to_owned()
        ]
    );
}

#[test]
fn release_source_is_idempotent_and_byte_identical() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let one = fixture.path("one");
    let two = fixture.path("two");
    write_release_source(&fixture.repo, &candidate, &one).expect("one");
    write_release_source(&fixture.repo, &candidate, &two).expect("two");
    write_release_source(&fixture.repo, &candidate, &one).expect("rerun");
    for file in [
        format!("rules_dx-{VERSION}.tar.gz"),
        format!("rules_dx-{VERSION}.tar.gz.sha256"),
        "source.json".to_owned(),
    ] {
        assert_eq!(
            std::fs::read(one.join(&file)).expect("one"),
            std::fs::read(two.join(&file)).expect("two"),
            "{file}"
        );
    }
}

#[test]
fn release_source_rejects_unknown_commit() {
    let fixture = Fixture::new();
    let candidate = Candidate::new(VERSION, &"0".repeat(40)).expect("candidate");
    let out = fixture.path("unknown");
    assert!(write_release_source(&fixture.repo, &candidate, &out).is_err());
    assert!(!out.join("source.json").exists());
}

#[test]
fn release_source_rejects_relative_and_conflicting_out_dir() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let error = write_release_source(&fixture.repo, &candidate, Path::new("relative"))
        .expect_err("relative");
    assert!(error.contains("must be absolute"), "{error}");
    let out = fixture.path("conflict");
    write(&out.join("source.json"), b"{}\n");
    let error = write_release_source(&fixture.repo, &candidate, &out).expect_err("conflict");
    assert!(error.contains("different bytes"), "{error}");
    assert!(!out.join(format!("rules_dx-{VERSION}.tar.gz")).exists());
}
