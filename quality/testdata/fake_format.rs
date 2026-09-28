#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Relpath,
    Json,
    Diff0,
    Warn,
    Diff,
}

#[derive(Default)]
struct Flags {
    fix: bool,
    check: bool,
    diff: bool,
    reformat: bool,
    pos_check: bool,
}

fn extension_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(idx) => &name[idx + 1..],
        None => name,
    }
}

fn mode_for(ext: &str) -> Mode {
    match ext {
        "cs" | "qml" => Mode::Relpath,
        "fs" => Mode::Json,
        "go" => Mode::Diff0,
        "css" | "less" | "scss" | "feature" | "sql" | "xml" => Mode::Warn,
        _ => Mode::Diff,
    }
}

fn wants_fix(ext: &str, flags: &Flags) -> bool {
    match ext {
        "proto" => !flags.diff,
        "html" => flags.reformat && !flags.check,
        "fs" => !flags.pos_check,
        "cs" | "qml" => !flags.pos_check && !flags.check,
        "scala" => !flags.check,
        _ => flags.fix,
    }
}

fn parse(argv: &[String]) -> (Flags, Vec<String>) {
    let mut flags = Flags::default();
    let mut files = Vec::new();
    let mut skip_next = false;
    for arg in argv.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        match arg.as_str() {
            "--config" | "--config-path" => skip_next = true,
            "--check" => flags.check = true,
            "--diff" | "-d" => flags.diff = true,
            "--reformat" => {
                flags.fix = true;
                flags.reformat = true;
            }
            "-w" | "--write" | "--fix" | "-i" => flags.fix = true,
            "check" => flags.pos_check = true,
            _ if arg.starts_with('-') => {}
            _ => {
                if Path::new(arg).is_file() {
                    files.push(arg.clone());
                }
            }
        }
    }
    (flags, files)
}

fn relpath(cwd: &Path, file: &str) -> String {
    let abs = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        cwd.join(file)
    };
    let cwd_parts: Vec<Component<'_>> = cwd.components().collect();
    let abs_parts: Vec<Component<'_>> = abs.components().collect();
    let mut common = 0;
    while common < cwd_parts.len()
        && common < abs_parts.len()
        && cwd_parts[common] == abs_parts[common]
    {
        common += 1;
    }
    if common == 0 {
        return file.to_owned();
    }
    let mut rel = PathBuf::new();
    for _ in common..cwd_parts.len() {
        rel.push("..");
    }
    for part in &abs_parts[common..] {
        rel.push(part);
    }
    if rel.as_os_str().is_empty() {
        return ".".to_owned();
    }
    rel.to_string_lossy().into_owned()
}

fn warn_rel(cwd: &str, file: &str) -> String {
    let mut prefix = cwd.to_owned();
    prefix.push('/');
    if let Some(rest) = file.strip_prefix(&prefix) {
        return rest.to_owned();
    }
    match file.rsplit('/').next() {
        Some(base) => base.to_owned(),
        None => file.to_owned(),
    }
}

fn is_dirty(path: &str) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => bytes.windows(6).any(|win| win == b"BADFMT"),
        Err(_) => false,
    }
}

fn rewrite_fixed(path: &str) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let mut out = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx..].starts_with(b"BADFMT") {
            out.extend_from_slice(b"fixed");
            idx += 6;
        } else {
            out.push(bytes[idx]);
            idx += 1;
        }
    }
    let tmp = format!("{path}.dxtmp");
    if std::fs::write(&tmp, &out).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn emit_diff(file: &str) {
    println!("--- a/{file}\n+++ b/{file}\n@@ -1 +1 @@\n-BADFMT\n+fixed");
}

pub fn run(argv: &[String]) -> i32 {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => return 2,
    };
    let (flags, files) = parse(argv);
    if files.is_empty() {
        return 0;
    }
    let ext = extension_of(&files[0]);
    if wants_fix(ext, &flags) {
        for file in &files {
            if is_dirty(file) {
                rewrite_fixed(file);
            }
        }
        return 0;
    }
    match mode_for(ext) {
        Mode::Relpath => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    println!("{}", relpath(&cwd, file));
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::Json => {
            let mut entries = Vec::new();
            let mut dirty = false;
            for file in &files {
                let rel = relpath(&cwd, file);
                if is_dirty(file) {
                    entries.push(format!(
                        "{{\"path\": \"{rel}\", \"status\": \"needs-formatting\"}}"
                    ));
                    dirty = true;
                } else {
                    entries.push(format!(
                        "{{\"path\": \"{rel}\", \"status\": \"unchanged\"}}"
                    ));
                }
            }
            println!("{{\"files\": [{}]}}", entries.join(", "));
            if dirty {
                99
            } else {
                0
            }
        }
        Mode::Warn => {
            let cwd_text = cwd.to_string_lossy().into_owned();
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    eprintln!("[warn] {}", warn_rel(&cwd_text, file));
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::Diff => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    emit_diff(file);
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::Diff0 => {
            for file in &files {
                if is_dirty(file) {
                    emit_diff(file);
                }
            }
            0
        }
    }
}
