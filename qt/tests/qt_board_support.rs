use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

pub const APP: &str = "//examples/qt-tasks/app:tasks";

pub fn manifest(variable: &str) -> hermetic::Manifest {
    let rel = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} names the manifest"));
    let text = std::fs::read_to_string(resolve_runfiles(&rel)).expect("manifest is readable");
    let base = std::env::current_dir().expect("runfiles cwd");
    hermetic::parse_manifest(&text, &base).expect("manifest parses")
}

pub struct Consumer {
    pub dx: PathBuf,
    pub repo: String,
    pub root: PathBuf,
    pub scratch: PathBuf,
    pub case: Case,
}

impl Consumer {
    pub fn new(name: &str, variable: &str, repo: Option<&str>) -> Self {
        let mut manifest = manifest(variable);
        let source = std::fs::canonicalize(&manifest.module_file)
            .expect("rules_dx MODULE.bazel resolves")
            .parent()
            .expect("rules_dx source tree")
            .to_path_buf();
        let case = nested::case(name);
        let scratch = case.root().to_path_buf();
        if let Some(repo) = repo {
            let fragment = read_text(&manifest.module_fragment)
                .replace(&format!("@{}//", manifest.repo_name), &format!("@{repo}//"));
            manifest.module_fragment = scratch.join("module_fragment.bazel");
            write(&manifest.module_fragment, &fragment);
            manifest.repo_name = repo.to_owned();
        }
        let root = scratch.join(&manifest.workspace_dir);
        hermetic::materialize(&manifest, &root, &source, "cc").expect("consumer materializes");
        let dx = std::fs::canonicalize(&manifest.dx).expect("dx resolves");
        Consumer {
            dx,
            repo: manifest.repo_name,
            root,
            scratch,
            case,
        }
    }

    pub fn dx(&self, command: &str, args: &[&str], rest: &[&str]) -> Run {
        let workspace = self.root.to_string_lossy().into_owned();
        let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
        argv.extend_from_slice(args);
        if !rest.is_empty() {
            argv.push("--");
            argv.extend_from_slice(rest);
        }
        self.case.run(&self.dx, &argv, &[])
    }

    pub fn tasks(&self, args: &[&str]) -> Run {
        self.dx("run", &[APP], args)
    }

    pub fn quality(&self, verb: &str, tool: &str, scope: &str) -> Run {
        let repo = &self.repo;
        let tool = format!("--@{repo}//config:tool_{tool}=@dx_qt//:{tool}");
        let policy = format!("--@{repo}//config:workspace=//:policy");
        self.dx(verb, &[scope], &[policy.as_str(), tool.as_str()])
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn edit(&self, rel: &str, from: &str, to: &str) -> String {
        let original = read_text(&self.path(rel));
        assert!(original.contains(from), "{rel} lacks {from:?}");
        write(&self.path(rel), &original.replacen(from, to, 1));
        original
    }
}

pub fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

pub fn rename(from: &Path, to: &Path) {
    std::fs::rename(from, to).unwrap_or_else(|error| panic!("rename {from:?}: {error}"));
}

pub fn assert_exit(result: &Run, code: i32) {
    assert_eq!(result.status.code(), Some(code), "{}", result.combined());
}

pub fn assert_fails(result: &Run) {
    assert_ne!(result.status.code(), Some(0), "{}", result.combined());
}

pub fn assert_has(text: &str, needle: &str) {
    assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
}

pub fn launch(app: &Path, args: &[&str]) -> Run {
    launch_with(app, args, &[])
}

pub fn launch_with(app: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let output = Command::new(app)
        .args(args)
        .env_clear()
        .envs(env.iter().copied())
        .current_dir(app.parent().expect("app dir"))
        .output()
        .expect("relocated app launches");
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

pub fn machine(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    dx_testing::image_machine(&bytes)
}

pub fn extract(consumer: &Consumer, package: &str, name: &str) -> PathBuf {
    let bin = consumer.path(&format!("bazel-bin/{package}"));
    let archive = format!("{name}.tar.gz");
    let tarball = std::fs::read(bin.join(&archive)).expect("tarball built");
    let checksum = read_text(&bin.join(format!("{archive}.sha256")));
    assert_eq!(
        checksum,
        format!("{}  {archive}\n", hermetic::sha256_hex(&tarball))
    );
    let elsewhere = consumer.scratch.join(name);
    std::fs::create_dir_all(&elsewhere).expect("extract dir");
    let copy = elsewhere.join(&archive);
    std::fs::write(&copy, &tarball).expect("tarball copied");
    let extracted = Command::new("tar")
        .arg("-xzf")
        .arg(&copy)
        .arg("-C")
        .arg(&elsewhere)
        .status()
        .expect("tar runs");
    assert!(extracted.success());
    elsewhere
}

pub fn pixel(image: &[u8], x: usize, y: usize) -> [u8; 3] {
    let header = b"P6\n320 72\n255\n";
    assert_eq!(&image[..header.len()], header, "not a 320x72 PPM");
    let at = header.len() + 3 * (y * 320 + x);
    [image[at], image[at + 1], image[at + 2]]
}

pub fn dark_pixels(image: &[u8], rows: std::ops::Range<usize>) -> usize {
    rows.flat_map(|y| (0..320).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(image, x, y).iter().all(|&channel| channel < 128))
        .count()
}

pub fn assert_board_pixels(image: &[u8]) {
    assert_eq!(pixel(image, 12, 36), [46, 125, 50], "done box");
    assert_eq!(pixel(image, 12, 60), [198, 40, 40], "open box");
    assert_eq!(pixel(image, 300, 12), [255, 255, 255], "background");
    assert!(dark_pixels(image, 0..24) > 40, "header text is blank");
}

pub const HOST_RUNTIME: &[&str] = &[
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
];

pub fn assert_bundled_only(trace: &str, root: &Path, plugin: &str) {
    let loaded: Vec<&str> = trace
        .lines()
        .filter_map(|line| line.split("calling init: ").nth(1))
        .collect();
    let root = root.to_string_lossy().into_owned();
    assert!(
        loaded
            .iter()
            .any(|path| path.ends_with(&format!("/resources/plugins/{plugin}"))),
        "{plugin} was not loaded: {loaded:?}"
    );
    let host: Vec<&&str> = loaded
        .iter()
        .filter(|path| !path.starts_with(root.as_str()))
        .filter(|path| {
            !HOST_RUNTIME
                .iter()
                .any(|name| path.ends_with(&format!("/{name}")))
        })
        .collect();
    assert!(host.is_empty(), "host libraries loaded: {host:?}");
}
