//! Reads a launch descriptor, resolves its runfiles and runs the program it names.
//!
//! The descriptor sits beside the executable as `<stem>.launch`, one entry per line:
//! `version=1`, `program=<key>`, `arg=<literal>`, `path=<key>`,
//! `prefixed=<prefix>\t<key>` and `env_flag=<flag>\t<VAR>`. The user's arguments
//! follow the listed ones.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_path::Resolver;

/// One argument of the program, in order.
#[derive(Debug, PartialEq, Eq)]
pub enum Item {
    /// A literal argument.
    Arg(String),
    /// The absolute path of a runfiles key.
    Path(String),
    /// A prefix followed by the absolute path of a runfiles key.
    Prefixed(String, String),
    /// A flag and the value of an environment variable, only when the variable is set and not empty.
    EnvFlag(String, String),
}

/// What one `<name>.launch` file describes.
#[derive(Debug, PartialEq, Eq)]
pub struct Descriptor {
    /// Runfiles key of the program.
    pub program: String,
    /// The arguments before the user's.
    pub items: Vec<Item>,
}

/// The runfiles source a child process is told about.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn pair(line: &str, value: &str) -> io::Result<(String, String)> {
    let (first, second) = value
        .split_once('\t')
        .ok_or_else(|| bad(format!("line needs a tab: {line}")))?;
    if second.is_empty() || second.contains('\t') {
        return Err(bad(format!("line needs one tab and a key: {line}")));
    }
    Ok((first.to_owned(), second.to_owned()))
}

/// Parses the line-based descriptor, rejecting anything unexpected.
pub fn parse(text: &str) -> io::Result<Descriptor> {
    let mut version = false;
    let mut program: Option<String> = None;
    let mut items = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if let Some(value) = line.strip_prefix("version=") {
            if value != "1" || version {
                return Err(bad(format!("unsupported descriptor line: {line}")));
            }
            version = true;
        } else if let Some(value) = line.strip_prefix("program=") {
            if value.is_empty() || program.is_some() {
                return Err(bad("descriptor needs exactly one program line"));
            }
            program = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("arg=") {
            items.push(Item::Arg(value.to_owned()));
        } else if let Some(value) = line.strip_prefix("path=") {
            if value.is_empty() {
                return Err(bad(format!("path line has an empty key: {line}")));
            }
            items.push(Item::Path(value.to_owned()));
        } else if let Some(value) = line.strip_prefix("prefixed=") {
            let (prefix, key) = pair(line, value)?;
            items.push(Item::Prefixed(prefix, key));
        } else if let Some(value) = line.strip_prefix("env_flag=") {
            let (flag, var) = pair(line, value)?;
            if flag.is_empty() {
                return Err(bad(format!("env_flag line has an empty flag: {line}")));
            }
            items.push(Item::EnvFlag(flag, var));
        } else {
            return Err(bad(format!("unknown descriptor line: {line}")));
        }
    }
    if !version {
        return Err(bad("descriptor has no version line"));
    }
    let program = program.ok_or_else(|| bad("descriptor needs exactly one program line"))?;
    Ok(Descriptor { program, items })
}

/// Names the descriptors that can sit beside one executable, best first.
///
/// A wrapper macro may rename the executable of an `<name>_upstream` rule, so
/// the descriptor of the `_upstream` target also counts.
pub fn descriptor_candidates(exe: &Path) -> Vec<PathBuf> {
    let mut found = vec![exe.with_extension("launch")];
    if let (Some(dir), Some(stem)) = (exe.parent(), exe.file_stem()) {
        let mut name = stem.to_os_string();
        name.push("_upstream.launch");
        found.push(dir.join(name));
    }
    found
}

/// Returns the first descriptor that exists beside one executable.
pub fn descriptor_path(exe: &Path) -> PathBuf {
    let candidates = descriptor_candidates(exe);
    candidates
        .iter()
        .find(|path| path.is_file())
        .or_else(|| candidates.first())
        .cloned()
        .unwrap_or_default()
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

/// Resolves one key, reading a file inside a tree artifact through the tree's own manifest entry.
fn locate(resolver: &Resolver, key: &str) -> io::Result<PathBuf> {
    let error = match resolver.lookup(key) {
        Ok(path) => return Ok(path),
        Err(error) => error,
    };
    let mut parent = key;
    while let Some((head, _)) = parent.rsplit_once('/') {
        if let Ok(tree) = resolver.lookup(head) {
            if tree.is_dir() {
                let rest = key.get(head.len() + 1..).unwrap_or_default();
                let path = tree.join(rest);
                if path.exists() {
                    return Ok(path);
                }
            }
        }
        parent = head;
    }
    Err(error)
}

/// Resolves every key of one descriptor into the program and its arguments.
pub fn plan(
    resolver: &Resolver,
    descriptor: &Descriptor,
    env: &dyn Fn(&str) -> Option<String>,
) -> io::Result<Plan> {
    let program = locate(resolver, &descriptor.program)?;
    let mut args: Vec<OsString> = Vec::new();
    for item in &descriptor.items {
        match item {
            Item::Arg(value) => args.push(value.into()),
            Item::Path(key) => args.push(locate(resolver, key)?.into_os_string()),
            Item::Prefixed(prefix, key) => {
                let mut value = OsString::from(prefix);
                value.push(locate(resolver, key)?);
                args.push(value);
            }
            Item::EnvFlag(flag, var) => {
                if let Some(value) = env(var).filter(|value| !value.is_empty()) {
                    args.push(flag.into());
                    args.push(value.into());
                }
            }
        }
    }
    Ok(Plan { program, args })
}

/// Returns the environment entry that tells the child where the runfiles are.
pub fn runfiles_env(resolver: &Resolver) -> (&'static str, PathBuf) {
    let source = resolver.source().to_path_buf();
    if source.is_dir() {
        ("RUNFILES_DIR", source)
    } else {
        ("RUNFILES_MANIFEST_FILE", source)
    }
}

/// Runs the program named by the descriptor beside `exe` and returns its exit code.
pub fn run(exe: &Path, user_args: &[String]) -> io::Result<i32> {
    let resolver = resolver_for(exe)?;
    let path = descriptor_path(exe);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))?;
    let descriptor = parse(&text)?;
    let plan = plan(&resolver, &descriptor, &|name| std::env::var(name).ok())?;
    let (name, value) = runfiles_env(&resolver);
    let status = Command::new(&plan.program)
        .args(&plan.args)
        .args(user_args)
        .env(name, value)
        .status()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot run {}: {error}", plan.program.display()),
            )
        })?;
    Ok(status.code().unwrap_or(1))
}

/// Runs the executable this process was started as.
pub fn run_from_env() -> io::Result<i32> {
    let argv: Vec<String> = std::env::args().collect();
    let exe = argv.first().ok_or_else(|| bad("no argv[0]"))?;
    run(Path::new(exe), argv.get(1..).unwrap_or_default())
}
