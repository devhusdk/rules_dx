//! Stages the files of one `rego_test` target and runs `opa test` over them.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_path::Resolver;

/// Names the environment entry holding the runfiles key of the descriptor.
pub const MANIFEST_ENV: &str = "DX_REGO_TEST_MANIFEST";

/// The staging plan one `rego_test` target writes.
#[derive(Debug, PartialEq, Eq)]
pub struct Descriptor {
    /// Runfiles key of the opa binary.
    pub opa: String,
    /// Runfiles key and workspace-relative destination of each staged file.
    pub files: Vec<(String, String)>,
}

/// What one `opa test` run printed and exited with.
#[derive(Debug)]
pub struct Outcome {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Parses the line-based descriptor, rejecting anything unexpected.
pub fn parse(text: &str) -> io::Result<Descriptor> {
    let mut version = false;
    let mut opa: Option<String> = None;
    let mut files: Vec<(String, String)> = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("version=") {
            if value != "1" || version {
                return Err(bad(format!("unsupported descriptor line: {line}")));
            }
            version = true;
        } else if let Some(value) = line.strip_prefix("opa=") {
            if value.is_empty() || opa.is_some() {
                return Err(bad("descriptor needs exactly one opa line"));
            }
            opa = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("file=") {
            let (key, dest) = value
                .split_once('\t')
                .ok_or_else(|| bad(format!("file line needs a tab: {line}")))?;
            check_dest(dest)?;
            if key.is_empty() {
                return Err(bad(format!("file line has an empty key: {line}")));
            }
            if files.iter().any(|(_, seen)| seen == dest) {
                return Err(bad(format!("duplicate destination {dest}")));
            }
            files.push((key.to_owned(), dest.to_owned()));
        } else {
            return Err(bad(format!("unknown descriptor line: {line}")));
        }
    }
    if !version {
        return Err(bad("descriptor has no version line"));
    }
    let opa = opa.ok_or_else(|| bad("descriptor needs exactly one opa line"))?;
    if files.is_empty() {
        return Err(bad("descriptor lists no files"));
    }
    Ok(Descriptor { opa, files })
}

fn check_dest(dest: &str) -> io::Result<()> {
    let invalid = dest.is_empty()
        || dest.contains('\\')
        || dest.contains(':')
        || dest
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if invalid {
        return Err(bad(format!("invalid destination {dest:?}")));
    }
    Ok(())
}

/// Builds the `opa` arguments for one run.
pub fn opa_args(test_only: Option<&str>, user_args: &[String]) -> Vec<String> {
    let mut args = vec![
        "test".to_owned(),
        "--fail-on-empty".to_owned(),
        "-v".to_owned(),
    ];
    if let Some(filter) = test_only.filter(|filter| !filter.is_empty()) {
        args.push("--run".to_owned());
        args.push(filter.to_owned());
    }
    args.extend(user_args.iter().cloned());
    args.push(".".to_owned());
    args
}

/// Finds the `.runfiles` directory above one executable path, when there is one.
fn tree_ancestor(exe: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in exe.components() {
        root.push(component.as_os_str());
        if component
            .as_os_str()
            .to_string_lossy()
            .ends_with(".runfiles")
        {
            return Some(root);
        }
    }
    None
}

/// Resolves the runfiles of the invoked executable without following symlinks.
pub fn resolver_for(exe: &Path) -> io::Result<Resolver> {
    let exe = if exe.is_absolute() {
        exe.to_path_buf()
    } else {
        std::env::current_dir()?.join(exe)
    };
    if let Some(tree) = tree_ancestor(&exe).filter(|tree| tree.is_dir()) {
        return Resolver::for_tree(&tree);
    }
    Resolver::for_binary(&exe)
}

/// Copies every described file into a fresh tree at `root`.
pub fn stage(resolver: &Resolver, descriptor: &Descriptor, root: &Path) -> io::Result<()> {
    if root.exists() {
        std::fs::remove_dir_all(root)?;
    }
    std::fs::create_dir_all(root)?;
    for (key, dest) in &descriptor.files {
        let source = resolver.lookup(key)?;
        let target = root.join(dest);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &target).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot stage {} as {dest}: {error}", source.display()),
            )
        })?;
    }
    Ok(())
}

/// Stages one target and runs `opa test` over the tree.
pub fn run(
    exe: &Path,
    manifest_key: &str,
    user_args: &[String],
    test_only: Option<&str>,
    scratch: &Path,
) -> io::Result<Outcome> {
    let resolver = resolver_for(exe)?;
    let manifest = resolver.lookup(manifest_key)?;
    let descriptor = parse(&std::fs::read_to_string(&manifest)?)?;
    let opa = resolver.lookup(&descriptor.opa).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "the opa binary {} is missing from the runfiles; set //config:tool_opa to a target-platform opa: {error}",
                descriptor.opa
            ),
        )
    })?;
    let root = scratch.join("rego_tree");
    stage(&resolver, &descriptor, &root)?;
    let output = Command::new(&opa)
        .args(opa_args(test_only, user_args))
        .current_dir(&root)
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot run {}: {error}", opa.display()),
            )
        })?;
    let code = output
        .status
        .code()
        .ok_or_else(|| bad("opa ended without an exit code"))?;
    Ok(Outcome {
        code: u8::try_from(code).unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Runs the target named by the process environment and prints opa's output.
pub fn run_from_env() -> io::Result<u8> {
    let argv: Vec<String> = std::env::args().collect();
    let exe = argv.first().ok_or_else(|| bad("no argv[0]"))?;
    let key = std::env::var(MANIFEST_ENV).map_err(|_| bad(format!("missing {MANIFEST_ENV}")))?;
    let test_only = std::env::var("TESTBRIDGE_TEST_ONLY").ok();
    let (scratch, temporary) = match std::env::var_os("TEST_TMPDIR") {
        Some(dir) => (PathBuf::from(dir), false),
        None => (
            std::env::temp_dir().join(format!("rego_test_{}", std::process::id())),
            true,
        ),
    };
    let result = run(
        Path::new(exe),
        &key,
        argv.get(1..).unwrap_or_default(),
        test_only.as_deref(),
        &scratch,
    );
    if temporary {
        let _ = std::fs::remove_dir_all(&scratch);
    }
    let outcome = result?;
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    Ok(outcome.code)
}
