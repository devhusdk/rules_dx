use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_path::Resolver;

#[derive(Debug, PartialEq, Eq)]
pub enum Item {
    Arg(String),
    Path(String),
    Prefixed(String, String),
    EnvFlag(String, String),
    Cwd(String, String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub program: String,
    pub fail: Option<String>,
    pub copy: Vec<(String, String)>,
    pub real_program: bool,
    pub items: Vec<Item>,
    pub unset: Vec<String>,
    pub env: Vec<(String, String)>,
    pub env_dir: Vec<(String, String)>,
    pub env_default: Vec<(String, String)>,
    pub verbs: Vec<String>,
    pub default_args: Vec<String>,
}

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

fn below(path: &str) -> bool {
    let path = Path::new(path);
    path.components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
        && path.file_name().is_some()
}

pub fn parse(text: &str) -> io::Result<Descriptor> {
    let mut program: Option<String> = None;
    let mut real_program = false;
    let mut items = Vec::new();
    let mut unset = Vec::new();
    let mut env = Vec::new();
    let mut env_dir = Vec::new();
    let mut env_default = Vec::new();
    let mut fail: Option<String> = None;
    let mut copy = Vec::new();
    let mut verbs = Vec::new();
    let mut default_args = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if let Some(value) = line
            .strip_prefix("program=")
            .or_else(|| line.strip_prefix("real_program="))
        {
            if value.is_empty() || program.is_some() {
                return Err(bad("descriptor needs exactly one program line"));
            }
            real_program = line.starts_with("real_program=");
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
        } else if let Some(value) = line.strip_prefix("cwd=") {
            let (before, after) = pair(line, value)?;
            items.push(Item::Cwd(before, after));
        } else if let Some(value) = line.strip_prefix("env_flag=") {
            let (flag, var) = pair(line, value)?;
            if flag.is_empty() {
                return Err(bad(format!("env_flag line has an empty flag: {line}")));
            }
            items.push(Item::EnvFlag(flag, var));
        } else if let Some(value) = line.strip_prefix("env=") {
            let (name, value) = pair(line, value)?;
            if name.is_empty() || name.contains('=') {
                return Err(bad(format!("env line has a bad name: {line}")));
            }
            env.push((name, value));
        } else if let Some(value) = line.strip_prefix("env_dir=") {
            let (name, key) = pair(line, value)?;
            if name.is_empty() || name.contains('=') {
                return Err(bad(format!("env_dir line has a bad name: {line}")));
            }
            env_dir.push((name, key));
        } else if let Some(value) = line.strip_prefix("env_default=") {
            let (name, value) = pair(line, value)?;
            if name.is_empty() || name.contains('=') {
                return Err(bad(format!("env_default line has a bad name: {line}")));
            }
            env_default.push((name, value));
        } else if let Some(value) = line.strip_prefix("verb=") {
            if value.is_empty() || value.contains('\t') {
                return Err(bad(format!("verb line has a bad word: {line}")));
            }
            verbs.push(value.to_owned());
        } else if let Some(value) = line.strip_prefix("default_arg=") {
            default_args.push(value.to_owned());
        } else if let Some(value) = line.strip_prefix("unset=") {
            if value.is_empty() || value.contains('=') || value.contains('\t') {
                return Err(bad(format!("unset line has a bad name: {line}")));
            }
            unset.push(value.to_owned());
        } else if let Some(value) = line.strip_prefix("fail=") {
            if value.is_empty() || fail.is_some() {
                return Err(bad("descriptor needs at most one fail line with a message"));
            }
            fail = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("copy=") {
            let (key, path) = pair(line, value)?;
            if key.is_empty() || !below(&path) {
                return Err(bad(format!(
                    "copy line needs a key and a path below the workspace: {line}"
                )));
            }
            copy.push((key, path));
        } else {
            return Err(bad(format!("unknown descriptor line: {line}")));
        }
    }
    if verbs.is_empty() && !default_args.is_empty() {
        return Err(bad("default_arg lines need a verb line"));
    }
    let bare = items.is_empty()
        && verbs.is_empty()
        && default_args.is_empty()
        && unset.is_empty()
        && env.is_empty()
        && env_dir.is_empty()
        && env_default.is_empty();
    let program = match (program, &fail, copy.is_empty()) {
        (Some(program), None, true) => program,
        (None, Some(_), true) | (None, None, false) if bare => String::new(),
        _ => {
            return Err(bad(
                "descriptor needs exactly one program line, one fail line or only copy lines",
            ));
        }
    };
    Ok(Descriptor {
        program,
        fail,
        copy,
        real_program,
        items,
        unset,
        env,
        env_dir,
        env_default,
        verbs,
        default_args,
    })
}

pub fn arguments(
    descriptor: &Descriptor,
    planned: Vec<OsString>,
    user_args: &[OsString],
) -> Vec<OsString> {
    let Some(default_verb) = descriptor.verbs.first() else {
        return planned
            .into_iter()
            .chain(user_args.iter().cloned())
            .collect();
    };
    if let Some((first, rest)) = user_args.split_first()
        && descriptor
            .verbs
            .iter()
            .any(|verb| first.as_os_str() == verb.as_str())
    {
        return std::iter::once(first.clone())
            .chain(planned)
            .chain(rest.iter().cloned())
            .collect();
    }
    std::iter::once(OsString::from(default_verb))
        .chain(planned)
        .chain(descriptor.default_args.iter().map(OsString::from))
        .chain(user_args.iter().cloned())
        .collect()
}

pub fn descriptor_candidates(exe: &Path) -> Vec<PathBuf> {
    let is_exe = exe
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
    let stem = if is_exe {
        exe.file_stem()
    } else {
        exe.file_name()
    };
    let Some(stem) = stem else {
        return vec![exe.with_extension("launch")];
    };
    [".launch", "_upstream.launch"]
        .iter()
        .map(|suffix| {
            let mut name = stem.to_os_string();
            name.push(suffix);
            exe.with_file_name(name)
        })
        .collect()
}

pub fn descriptor_path(exe: &Path) -> PathBuf {
    let candidates = descriptor_candidates(exe);
    candidates
        .iter()
        .find(|path| path.is_file())
        .or_else(|| candidates.first())
        .cloned()
        .unwrap_or_default()
}

fn output_key(path: &Path) -> Option<String> {
    let parts: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let bin = (2..parts.len()).rev().find(|&index| {
        parts.get(index).is_some_and(|part| part == "bin")
            && parts.get(index - 2).is_some_and(|part| part == "bazel-out")
    })?;
    let package = parts.get(bin + 1..)?;
    match package.split_first() {
        Some((first, rest)) if first == "external" && !rest.is_empty() => Some(rest.join("/")),
        Some(_) => Some(format!("_main/{}", package.join("/"))),
        None => None,
    }
}

pub fn descriptor_keys(exe: &Path) -> Vec<String> {
    descriptor_candidates(exe)
        .iter()
        .filter_map(|candidate| output_key(candidate))
        .collect()
}

pub fn descriptor_for(resolver: &Resolver, exe: &Path) -> PathBuf {
    let beside = descriptor_path(exe);
    if beside.is_file() {
        return beside;
    }
    descriptor_keys(exe)
        .iter()
        .filter_map(|key| resolver.lookup(key).ok())
        .find(|path| path.is_file())
        .unwrap_or(beside)
}

fn locate(resolver: &Resolver, key: &str) -> io::Result<PathBuf> {
    let error = match resolver.lookup(key) {
        Ok(path) => return Ok(path),
        Err(error) => error,
    };
    let mut parent = key;
    while let Some((head, _)) = parent.rsplit_once('/') {
        if let Ok(tree) = resolver.lookup(head)
            && tree.is_dir()
        {
            let rest = key.get(head.len() + 1..).unwrap_or_default();
            let path = tree.join(rest);
            if path.exists() {
                return Ok(path);
            }
        }
        parent = head;
    }
    Err(error)
}

pub fn plan(
    resolver: &Resolver,
    descriptor: &Descriptor,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> io::Result<Plan> {
    let mut program = locate(resolver, &descriptor.program)?;
    if descriptor.real_program {
        program = dx_path::without_verbatim(&program.canonicalize()?);
    }
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
            Item::Cwd(before, after) => {
                let mut value = OsString::from(before);
                value.push(std::env::current_dir()?);
                value.push(after);
                args.push(value);
            }
            Item::EnvFlag(flag, var) => {
                if let Some(value) = env(var).filter(|value| !value.is_empty()) {
                    args.push(flag.into());
                    args.push(value);
                }
            }
        }
    }
    Ok(Plan { program, args })
}

pub fn child_env(
    resolver: &Resolver,
    descriptor: &Descriptor,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> io::Result<Vec<(String, String)>> {
    let mut out = descriptor.env.clone();
    for (name, key) in &descriptor.env_dir {
        let file = locate(resolver, key)?;
        let dir = file.parent().unwrap_or(&file);
        out.push((name.clone(), dir.to_string_lossy().into_owned()));
    }
    for (name, value) in &descriptor.env_default {
        if env(name).is_none_or(|current| current.is_empty()) {
            out.push((name.clone(), value.clone()));
        }
    }
    Ok(out)
}

pub fn runfiles_env(resolver: &Resolver) -> (&'static str, PathBuf) {
    let source = resolver.source().to_path_buf();
    if source.is_dir() {
        ("RUNFILES_DIR", source)
    } else {
        ("RUNFILES_MANIFEST_FILE", source)
    }
}

pub fn command(
    plan: &Plan,
    unset: &[String],
    env: &[(String, String)],
    runfiles: (&str, PathBuf),
    user_args: &[OsString],
) -> Command {
    let mut command = Command::new(&plan.program);
    command.args(&plan.args).args(user_args);
    for name in unset {
        command.env_remove(name);
    }
    command.envs(env.iter().map(|(name, value)| (name, value)));
    command.env(runfiles.0, runfiles.1);
    command
}

pub fn copy_into(
    resolver: &Resolver,
    copy: &[(String, String)],
    workspace: &Path,
) -> io::Result<()> {
    for (key, path) in copy {
        let source = locate(resolver, key)?;
        let dest = workspace.join(path);
        let describe =
            |error: io::Error| io::Error::new(error.kind(), format!("{}: {error}", dest.display()));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(describe)?;
        }
        let bytes = std::fs::read(&source).map_err(|error| {
            io::Error::new(error.kind(), format!("{}: {error}", source.display()))
        })?;
        std::fs::write(&dest, bytes).map_err(describe)?;
    }
    Ok(())
}

pub fn run(exe: &Path, user_args: &[OsString]) -> io::Result<i32> {
    let resolver = Resolver::for_executable(exe);
    let path = match &resolver {
        Ok(resolver) => descriptor_for(resolver, exe),
        Err(_) => descriptor_path(exe),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return Err(match resolver {
                Err(missing) => missing,
                Ok(_) => io::Error::new(error.kind(), format!("{}: {error}", path.display())),
            });
        }
    };
    let descriptor = parse(&text)?;
    if let Some(message) = &descriptor.fail {
        dx_error::stdio::eprint_line(format_args!("{message}"));
        return Ok(2);
    }
    let resolver = resolver?;
    if !descriptor.copy.is_empty() {
        let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                bad("BUILD_WORKSPACE_DIRECTORY is not set: start this with bazel run")
            })?;
        copy_into(&resolver, &descriptor.copy, Path::new(&workspace))?;
        return Ok(0);
    }
    let mut plan = plan(&resolver, &descriptor, &|name| std::env::var_os(name))?;
    plan.args = arguments(&descriptor, std::mem::take(&mut plan.args), user_args);
    let child = child_env(&resolver, &descriptor, &|name| std::env::var_os(name))?;
    let status = command(
        &plan,
        &descriptor.unset,
        &child,
        runfiles_env(&resolver),
        &[],
    )
    .status()
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot run {}: {error}", plan.program.display()),
        )
    })?;
    Ok(status.code().unwrap_or(1))
}

pub fn run_from_env() -> io::Result<i32> {
    let argv: Vec<OsString> = std::env::args_os().collect();
    let exe = argv.first().ok_or_else(|| bad("no argv[0]"))?;
    run(Path::new(exe), argv.get(1..).unwrap_or_default())
}
