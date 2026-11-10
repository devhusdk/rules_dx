//! Creates the directory for a new language and registers it in families.json.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const PLATFORMS: [&str; 5] = [
    "linux_arm64",
    "linux_x86_64",
    "macos_arm64",
    "windows_arm64",
    "windows_x86_64",
];

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn title_of(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn manifest(name: &str, title: &str) -> String {
    let platforms = PLATFORMS
        .iter()
        .map(|platform| format!("    \"{platform}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"$schema\": \"../modules/lang.schema.json\",\n  \"name\": \"{name}\",\n  \"title\": \"{title}\",\n  \"family\": \"core\",\n  \"ecosystems\": [],\n  \"manifests\": [],\n  \"lockfiles\": [],\n  \"tools\": [],\n  \"platforms\": [\n{platforms}\n  ],\n  \"pins\": [\n    \"pins.json\"\n  ],\n  \"entry_points\": []\n}}\n"
    )
}

fn files(name: &str, title: &str) -> Vec<(String, String)> {
    let public = "package(default_visibility = [\"//visibility:public\"])\n";
    vec![
        (
            "defs.bzl".to_owned(),
            format!("\"\"\"Public {title} rules and macros.\"\"\"\n"),
        ),
        (
            "extensions.bzl".to_owned(),
            format!("\"\"\"Public {title} module extensions.\"\"\"\n"),
        ),
        (
            "BUILD.bazel".to_owned(),
            format!(
                "{public}\ntoolchain_type(name = \"toolchain_type\")\n\nexports_files([\n    \"defs.bzl\",\n    \"extensions.bzl\",\n    \"lang.json\",\n    \"pins.json\",\n])\n"
            ),
        ),
        ("lang.json".to_owned(), manifest(name, title)),
        ("pins.json".to_owned(), "{\n  \"tools\": {}\n}\n".to_owned()),
        (
            "private/BUILD.bazel".to_owned(),
            format!("{public}\nexports_files([\"defs.bzl\"])\n"),
        ),
        (
            "private/defs.bzl".to_owned(),
            format!("\"\"\"{title} rule internals.\"\"\"\n"),
        ),
        (
            "tests/BUILD.bazel".to_owned(),
            format!("{public}\nfilegroup(\n    name = \"fixtures\",\n    srcs = glob([\"fixtures/**\"], allow_empty = True),\n)\n"),
        ),
    ]
}

fn register(families: &str, name: &str) -> Result<String, String> {
    let lines: Vec<&str> = families.lines().collect();
    let core = lines
        .iter()
        .position(|line| line.trim() == "\"core\": {")
        .ok_or("families.json has no core family")?;
    let open = lines[core..]
        .iter()
        .position(|line| line.trim() == "\"dirs\": [")
        .map(|offset| core + offset)
        .ok_or("core family has no dirs")?;
    let close = lines[open..]
        .iter()
        .position(|line| line.trim_start().starts_with(']'))
        .map(|offset| open + offset)
        .ok_or("core dirs is not closed")?;
    let mut dirs: Vec<String> = lines[open + 1..close]
        .iter()
        .map(|line| line.trim().trim_end_matches(',').trim_matches('"').to_owned())
        .collect();
    if dirs.iter().any(|dir| dir == name) {
        return Err(format!("{name} is already in families.json"));
    }
    dirs.push(name.to_owned());
    dirs.sort();
    let indent = &lines[open + 1][..lines[open + 1].len() - lines[open + 1].trim_start().len()];
    let body: Vec<String> = dirs
        .iter()
        .enumerate()
        .map(|(index, dir)| {
            let comma = if index + 1 < dirs.len() { "," } else { "" };
            format!("{indent}\"{dir}\"{comma}")
        })
        .collect();
    let mut out: Vec<String> = lines[..=open].iter().map(|l| (*l).to_owned()).collect();
    out.extend(body);
    out.extend(lines[close..].iter().map(|l| (*l).to_owned()));
    Ok(out.join("\n") + "\n")
}

fn scaffold(root: &Path, name: &str, title: &str) -> Result<Vec<String>, String> {
    if !valid_name(name) {
        return Err(format!("{name}: use lowercase letters, digits and _"));
    }
    let dir = root.join(name);
    if dir.exists() {
        return Err(format!("{name}: already exists"));
    }
    let families_path = root.join("modules/families.json");
    let families = fs::read_to_string(&families_path)
        .map_err(|error| format!("{}: {error}", families_path.display()))?;
    let updated = register(&families, name)?;
    let mut written = Vec::new();
    for (relative, content) in files(name, title) {
        let path = dir.join(&relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::write(&path, content).map_err(|error| format!("{}: {error}", path.display()))?;
        written.push(format!("{name}/{relative}"));
    }
    fs::write(&families_path, updated)
        .map_err(|error| format!("{}: {error}", families_path.display()))?;
    written.push("modules/families.json".to_owned());
    Ok(written)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(name) = args.next() else {
        eprintln!("usage: bazel run //tools/new_language -- <lang> [title]");
        return ExitCode::from(2);
    };
    let Some(root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty())
    else {
        eprintln!("new_language: BUILD_WORKSPACE_DIRECTORY is unset; run it with `bazel run //tools/new_language`");
        return ExitCode::from(2);
    };
    let title = args.next().unwrap_or_else(|| title_of(&name));
    match scaffold(Path::new(&root), &name, &title) {
        Ok(written) => {
            for path in written {
                println!("{path}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("new_language: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAMILIES: &str = "{\n  \"families\": {\n    \"core\": {\n      \"dirs\": [\n        \"ansible\",\n        \"zig\"\n      ],\n      \"x\": []\n    }\n  }\n}\n";

    fn scratch(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("new_language_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("modules")).expect("modules");
        fs::write(root.join("modules/families.json"), FAMILIES).expect("families");
        root
    }

    #[test]
    fn creates_the_layout_and_registers_the_dir() {
        let root = scratch("ok");
        let written = scaffold(&root, "mylang", "MyLang").expect("scaffold");
        for path in [
            "defs.bzl",
            "extensions.bzl",
            "BUILD.bazel",
            "lang.json",
            "pins.json",
            "private/BUILD.bazel",
            "private/defs.bzl",
            "tests/BUILD.bazel",
        ] {
            assert!(root.join("mylang").join(path).is_file(), "{path}");
        }
        assert!(written.contains(&"modules/families.json".to_owned()));
        let families = fs::read_to_string(root.join("modules/families.json")).expect("read");
        assert!(families.contains("\"ansible\",\n        \"mylang\",\n        \"zig\"\n"));
        let lang = fs::read_to_string(root.join("mylang/lang.json")).expect("lang");
        assert!(lang.contains("\"name\": \"mylang\""));
        assert!(lang.contains("\"title\": \"MyLang\""));
        assert!(lang.contains("\"windows_x86_64\"\n  ]"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_bad_and_existing_names() {
        let root = scratch("bad");
        assert!(scaffold(&root, "Bad-Name", "x").is_err());
        fs::create_dir_all(root.join("taken")).expect("taken");
        assert!(scaffold(&root, "taken", "x").is_err());
        scaffold(&root, "fresh", "Fresh").expect("first");
        assert!(scaffold(&root, "fresh", "Fresh").is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
