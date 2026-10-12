use std::path::Path;

use super::{AdoptError, ScaffoldFile};

pub const SUPPORTED_NEW_LANGUAGES: &[&str] = &[
    "rust",
    "python",
    "javascript",
    "typescript",
    "go",
    "java",
    "kotlin",
    "scala",
    "csharp",
    "fsharp",
    "c",
    "cc",
    "cpp",
    "rust-web",
];

pub const RUST_WEB_EDITION: &str = "2021";
pub const RUST_WEB_WASM_BINDGEN_CRATE_VERSION: &str = "0.2.121";
pub const RULES_RUST_WASM_BINDGEN_VERSION: &str = "0.74.0";
pub const RUST_WEB_PLATFORMS_VERSION: &str = "1.1.0";
pub const RUST_WEB_BAZEL_VERSION: &str = "9.2.0";
pub const RUST_WEB_WASM_REPOS: &[&str] = &[
    "chrome",
    "chrome_headless_shell",
    "chrome_headless_shell_linux64",
    "chrome_headless_shell_mac_arm64",
    "chrome_headless_shell_mac_x64",
    "chrome_headless_shell_win32",
    "chrome_headless_shell_win64",
    "chrome_linux64",
    "chrome_mac_arm64",
    "chrome_mac_x64",
    "chrome_win32",
    "chrome_win64",
    "chromedriver",
    "chromedriver_linux64",
    "chromedriver_mac_arm64",
    "chromedriver_mac_x64",
    "chromedriver_win32",
    "chromedriver_win64",
    "firefox",
    "firefox_linux_aarch64",
    "firefox_linux_x86_64",
    "firefox_local",
    "firefox_mac",
    "geckodriver",
    "geckodriver_linux64",
    "geckodriver_linux_aarch64",
    "geckodriver_macos",
    "geckodriver_macos_aarch64",
    "geckodriver_win64",
    "rrwbd",
    "rrwbd__serde_json-1.0.145",
    "rrwbd__wasm-bindgen-0.2.121",
    "rrwbd__wasm-bindgen-cli-0.2.121",
    "rrwbd__wasm-bindgen-test-0.3.71",
    "safaridriver",
];

pub const NEW_LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("c", "cpp"),
    ("cc", "cpp"),
    ("c#", "csharp"),
    ("f#", "fsharp"),
];

pub fn new_language_name_list() -> String {
    SUPPORTED_NEW_LANGUAGES.join(", ")
}

pub fn normalize_new_language(language: &str) -> Option<&'static str> {
    if let Some((_, canonical)) = NEW_LANGUAGE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == language)
    {
        return Some(canonical);
    }
    SUPPORTED_NEW_LANGUAGES
        .iter()
        .copied()
        .find(|name| *name == language)
}

pub fn new_is_known_language(language: &str) -> bool {
    normalize_new_language(language).is_some()
}

pub fn default_new_name() -> &'static str {
    "my_project"
}

pub fn plan_new_files(language: &str, name: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let canonical =
        normalize_new_language(language).ok_or_else(|| AdoptError::NewUnknownLanguage {
            language: language.to_owned(),
        })?;
    let destination = validate_new_destination(name)?;
    let identity = derive_new_identity(canonical, &destination)?;
    let mut files = Vec::new();
    for file in super::plan_init_files(&identity)? {
        files.push(ScaffoldFile {
            path: format!("{destination}/{}", file.path),
            content: file.content,
        });
    }
    for (suffix, content) in new_language_files(canonical, &identity)? {
        files.push(ScaffoldFile {
            path: format!("{destination}/{suffix}"),
            content,
        });
    }
    Ok(files)
}

const RESERVED_DESTINATION_STEMS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn destination_segment_reason(segment: &str) -> Option<String> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Some(
            "destinations stay inside the workspace (no empty, '.' or '..' segments)".to_owned(),
        );
    }
    if segment.chars().any(|c| c.is_control()) {
        return Some("destinations never contain control characters".to_owned());
    }
    if segment.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return Some("destinations never contain '<', '>', ':', '\"', '|', '?' or '*'".to_owned());
    }
    if segment.ends_with(' ') || segment.ends_with('.') {
        return Some("destination segments never end with a space or '.'".to_owned());
    }
    let stem = segment.split('.').next().unwrap_or(segment);
    if RESERVED_DESTINATION_STEMS
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Some(format!(
            "destination segment {segment:?} is a reserved device name"
        ));
    }
    None
}

pub fn validate_new_destination(name: &str) -> Result<String, AdoptError> {
    if name.is_empty() {
        return Ok(default_new_name().to_owned());
    }
    let refused = |reason: String| AdoptError::NewInvalidDestination {
        name: name.to_owned(),
        reason,
    };
    if name.contains('\\') {
        return Err(refused(
            "destinations never contain '\\' (use '/' for nesting)".to_owned(),
        ));
    }
    if name.starts_with('/') {
        return Err(refused(
            "destinations stay inside the workspace (no absolute paths)".to_owned(),
        ));
    }
    if name.len() >= 2 && name.as_bytes()[1] == b':' && name.as_bytes()[0].is_ascii_alphabetic() {
        return Err(refused(
            "destinations stay inside the workspace (no absolute paths)".to_owned(),
        ));
    }
    for segment in name.split('/') {
        if let Some(reason) = destination_segment_reason(segment) {
            return Err(refused(reason));
        }
    }
    Ok(name.to_owned())
}

fn fold_identity_text(stem: &str) -> String {
    let mut folded = String::with_capacity(stem.len());
    let mut gap = true;
    for c in stem.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
            folded.push(c);
            gap = true;
        } else if gap {
            folded.push('-');
            gap = false;
        }
    }
    folded.trim_matches(['-', '_', '.']).to_owned()
}

pub fn derive_new_identity(canonical: &str, destination: &str) -> Result<String, AdoptError> {
    let stem = destination.rsplit('/').next().unwrap_or(destination);
    let identity = fold_identity_text(stem);
    let check = match canonical {
        "rust" | "rust-web" => dx_identity::validate_cargo(&identity),
        "python" => {
            dx_identity::validate_dotted(&identity, "python project names use [A-Za-z0-9_.-] only")
        }
        "javascript" | "typescript" => dx_identity::validate_npm(&identity),
        "go" => dx_identity::validate_go(&identity),
        "java" | "kotlin" | "scala" => {
            dx_identity::validate_dotted(&identity, "maven artifact ids use [A-Za-z0-9_.-] only")
        }
        "csharp" | "fsharp" => dx_identity::validate_nuget(&identity),
        _ => Ok(()),
    };
    check.map_err(|reason| AdoptError::NewInvalidIdentity {
        language: canonical.to_owned(),
        name: identity.clone(),
        reason: reason.to_owned(),
    })?;
    Ok(identity)
}

fn escape_json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn escape_sbt_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn cargo_manifest(identity: &str) -> Result<String, AdoptError> {
    let mut package = toml::Table::new();
    package.insert("name".to_owned(), toml::Value::String(identity.to_owned()));
    package.insert(
        "version".to_owned(),
        toml::Value::String("0.1.0".to_owned()),
    );
    package.insert("edition".to_owned(), toml::Value::String("2021".to_owned()));
    let mut root = toml::Table::new();
    root.insert("package".to_owned(), toml::Value::Table(package));
    toml::to_string(&root).map_err(|e| AdoptError::NewInvalidIdentity {
        language: "rust".to_owned(),
        name: identity.to_owned(),
        reason: e.to_string(),
    })
}

fn rust_web_cargo_manifest(identity: &str) -> Result<String, AdoptError> {
    let mut package = toml::Table::new();
    package.insert("name".to_owned(), toml::Value::String(identity.to_owned()));
    package.insert(
        "version".to_owned(),
        toml::Value::String("0.1.0".to_owned()),
    );
    package.insert(
        "edition".to_owned(),
        toml::Value::String(RUST_WEB_EDITION.to_owned()),
    );
    let mut dependencies = toml::Table::new();
    dependencies.insert(
        "wasm-bindgen".to_owned(),
        toml::Value::String(format!("={RUST_WEB_WASM_BINDGEN_CRATE_VERSION}")),
    );
    let mut root = toml::Table::new();
    root.insert("package".to_owned(), toml::Value::Table(package));
    root.insert("dependencies".to_owned(), toml::Value::Table(dependencies));
    toml::to_string(&root).map_err(|e| AdoptError::NewInvalidIdentity {
        language: "rust-web".to_owned(),
        name: identity.to_owned(),
        reason: e.to_string(),
    })
}

fn rust_web_crate_ident(identity: &str) -> String {
    identity.replace('-', "_")
}

fn rust_web_lib_rs() -> String {
    "use wasm_bindgen::prelude::*;\n\npub fn greeting(name: &str) -> String {\n    format!(\"Hello from {name}!\")\n}\n\n#[wasm_bindgen]\npub fn greet(name: &str) -> String {\n    greeting(name)\n}\n\n#[wasm_bindgen]\npub fn add(left: u32, right: u32) -> u32 {\n    left + right\n}\n\n#[cfg(test)]\nmod tests {\n    use super::{add, greeting};\n\n    #[test]\n    fn greeting_mentions_name() {\n        assert!(greeting(\"dx\").contains(\"dx\"));\n    }\n\n    #[test]\n    fn add_sums() {\n        assert_eq!(add(40, 2), 42);\n    }\n}\n".to_owned()
}

fn rust_web_main_rs(identity: &str) -> String {
    format!(
        "fn main() {{\n    println!(\"{{}}\", {}::greeting(\"native\"));\n}}\n",
        rust_web_crate_ident(identity)
    )
}

fn rust_web_build_bazel(identity: &str) -> String {
    let crate_ident = rust_web_crate_ident(identity);
    format!(
        "# gazelle:resolve rust wasm_bindgen @rules_rust_wasm_bindgen//3rdparty:wasm_bindgen\nload(\"@rules_dx//rust/rules:defs.bzl\", \"rust_binary\", \"rust_clippy_test\", \"rust_library\", \"rust_test\", \"rust_wasm_bindgen\", \"rust_wasm_bindgen_test\", \"rustfmt_test\")\n\nrust_library(\n    name = \"{identity}\",\n    srcs = [\"src/lib.rs\"],\n    crate_name = \"{crate_ident}\",\n    crate_root = \"src/lib.rs\",\n    edition = \"{edition}\",\n    deps = [\"@rules_rust_wasm_bindgen//3rdparty:wasm_bindgen\"],\n)\n\nrust_binary(\n    name = \"{identity}_bin\",\n    srcs = [\"src/main.rs\"],\n    crate_name = \"{crate_ident}_bin\",\n    crate_root = \"src/main.rs\",\n    edition = \"{edition}\",\n    deps = [\":{identity}\"],\n)\n\nrust_test(\n    name = \"{identity}_test\",\n    crate = \":{identity}\",\n)\n\nrustfmt_test(\n    name = \"{identity}_fmt_test\",\n    size = \"small\",\n    targets = [\n        \":{identity}\",\n        \":{identity}_bin\",\n    ],\n)\n\nrust_clippy_test(\n    name = \"{identity}_clippy_test\",\n    size = \"small\",\n    targets = [\n        \":{identity}\",\n        \":{identity}_bin\",\n    ],\n)\n\nrust_wasm_bindgen(\n    name = \"{identity}_web\",\n    target = \"web\",\n    wasm_file = \":{identity}_bin_upstream\",\n)\n\nrust_wasm_bindgen_test(\n    name = \"{identity}_browser_test\",\n    tags = [\"manual\"],\n    wasm = \":{identity}_web\",\n)\n",
        edition = RUST_WEB_EDITION,
    )
}

fn rust_web_module_bazel(identity: &str) -> String {
    let mut repos = String::new();
    for repo in RUST_WEB_WASM_REPOS {
        repos.push_str(&format!("    \"{repo}\",\n"));
    }
    format!(
        "module(name = \"{identity}\")\n\nbazel_dep(name = \"rules_dx\", version = \"{dx}\")\nbazel_dep(name = \"rules_rust_wasm_bindgen\", version = \"{bindgen}\")\nbazel_dep(name = \"platforms\", version = \"{platforms}\")\n\nwasm_bindgen = use_extension(\"@rules_rust_wasm_bindgen//:extensions.bzl\", \"rust_ext\")\nuse_repo(\n    wasm_bindgen,\n{repos})\n",
        dx = super::DX_VERSION,
        bindgen = RULES_RUST_WASM_BINDGEN_VERSION,
        platforms = RUST_WEB_PLATFORMS_VERSION,
    )
}

fn rust_web_readme(identity: &str) -> String {
    format!(
        "# {identity}\n\nShared Rust core with a native binary and a browser build.\n\n```sh\nbazel build //...\nbazel test //...\n```\n\nRun the browser test with a local driver:\n\n```sh\ngeckodriver --port 4444 &\nbazel test :{identity}_browser_test --test_env=GECKODRIVER_REMOTE=http://127.0.0.1:4444\n```\n\nThe browser test is manual: it needs Firefox and geckodriver on a\nqualified host. Mobile templates are not included.\n"
    )
}

fn rust_web_files(identity: &str) -> Result<Vec<(String, String)>, AdoptError> {
    Ok(vec![
        ("Cargo.toml".to_owned(), rust_web_cargo_manifest(identity)?),
        ("src/lib.rs".to_owned(), rust_web_lib_rs()),
        ("src/main.rs".to_owned(), rust_web_main_rs(identity)),
        ("BUILD.bazel".to_owned(), rust_web_build_bazel(identity)),
        ("MODULE.bazel".to_owned(), rust_web_module_bazel(identity)),
        (
            ".bazelversion".to_owned(),
            format!("{}\n", RUST_WEB_BAZEL_VERSION),
        ),
        ("README.md".to_owned(), rust_web_readme(identity)),
    ])
}

fn python_manifest(identity: &str) -> Result<String, AdoptError> {
    let mut project = toml::Table::new();
    project.insert("name".to_owned(), toml::Value::String(identity.to_owned()));
    project.insert(
        "version".to_owned(),
        toml::Value::String("0.1.0".to_owned()),
    );
    project.insert(
        "requires-python".to_owned(),
        toml::Value::String(">=3.12".to_owned()),
    );
    project.insert("dependencies".to_owned(), toml::Value::Array(Vec::new()));
    let mut root = toml::Table::new();
    root.insert("project".to_owned(), toml::Value::Table(project));
    toml::to_string(&root).map_err(|e| AdoptError::NewInvalidIdentity {
        language: "python".to_owned(),
        name: identity.to_owned(),
        reason: e.to_string(),
    })
}

fn new_language_files(
    canonical: &str,
    identity: &str,
) -> Result<Vec<(String, String)>, AdoptError> {
    let json_name = escape_json_string(identity);
    let xml_name = escape_xml(identity);
    let sbt_name = escape_sbt_string(identity);
    let files = match canonical {
        "rust" => vec![
            ("Cargo.toml".to_owned(), cargo_manifest(identity)?),
            (
                "src/main.rs".to_owned(),
                "fn main() {\n    println!(\"hello world\");\n}\n".to_owned(),
            ),
        ],
        "rust-web" => rust_web_files(identity)?,
        "python" => vec![
            ("pyproject.toml".to_owned(), python_manifest(identity)?),
            (
                "hello.py".to_owned(),
                "\"\"\"Greeting helper with no dependencies.\"\"\"\n\n\ndef greet(name):\n    \"\"\"Return a greeting for name.\"\"\"\n    return f\"Hello, {{name}}!\"\n".to_owned(),
            ),
        ],
        "javascript" => vec![
            (
                "package.json".to_owned(),
                format!(
                    "{{\n  \"name\": \"{json_name}\",\n  \"private\": true,\n  \"type\": \"module\"\n}}\n"
                ),
            ),
            (
                "hello.js".to_owned(),
                "export function hello(name) {\n\treturn `hello ${name}`;\n}\n".to_owned(),
            ),
        ],
        "typescript" => vec![
            (
                "package.json".to_owned(),
                format!(
                    "{{\n  \"name\": \"{json_name}\",\n  \"private\": true,\n  \"type\": \"module\"\n}}\n"
                ),
            ),
            (
                "tsconfig.json".to_owned(),
                "{\n  \"compilerOptions\": {\n    \"module\": \"ESNext\",\n    \"target\": \"ES2022\",\n    \"strict\": true\n  }\n}\n"
                    .to_owned(),
            ),
            (
                "hello.ts".to_owned(),
                "export function hello(name: string): string {\n\treturn `hello ${name}`;\n}\n"
                    .to_owned(),
            ),
        ],
        "go" => vec![
            (
                "go.mod".to_owned(),
                format!("module {identity}\n\ngo 1.26\n"),
            ),
            (
                "hello.go".to_owned(),
                "package hello\n\n// Hello returns a greeting for name.\nfunc Hello(name string) string {\n\treturn \"hello \" + name\n}\n"
                    .to_owned(),
            ),
        ],
        "java" => vec![
            (
                "pom.xml".to_owned(),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>example.com</groupId>\n  <artifactId>{xml_name}</artifactId>\n  <version>0.1.0</version>\n  <packaging>jar</packaging>\n</project>\n"
                ),
            ),
            (
                "Hello.java".to_owned(),
                "package hello;\n\npublic class Hello {\n  public static String hello(String name) {\n    return \"hello \" + name;\n  }\n}\n"
                    .to_owned(),
            ),
        ],
        "kotlin" => vec![
            (
                "pom.xml".to_owned(),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>example.com</groupId>\n  <artifactId>{xml_name}</artifactId>\n  <version>0.1.0</version>\n  <packaging>jar</packaging>\n</project>\n"
                ),
            ),
            (
                "Hello.kt".to_owned(),
                "package hello\n\nobject Hello {\n  fun hello(name: String): String {\n    return \"hello \" + name\n  }\n}\n"
                    .to_owned(),
            ),
        ],
        "scala" => vec![
            (
                "build.sbt".to_owned(),
                format!(
                    "ThisBuild / scalaVersion := \"2.13.18\"\nThisBuild / organization := \"example.com\"\nlazy val root = (project in file(\".\")).settings(name := \"{sbt_name}\")\n"
                ),
            ),
            (
                "Hello.scala".to_owned(),
                "package hello\n\nobject Hello {\n  def hello(name: String): String = \"hello \" + name\n}\n"
                    .to_owned(),
            ),
        ],
        "csharp" => vec![
            (
                format!("{identity}.csproj"),
                "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net10.0</TargetFramework>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n</Project>\n"
                    .to_owned(),
            ),
            (
                "Hello.cs".to_owned(),
                "namespace Hello;\n\npublic static class Greeter\n{\n    public static string Greet(string name)\n    {\n        return \"hello \" + name;\n    }\n}\n"
                    .to_owned(),
            ),
        ],
        "fsharp" => vec![
            (
                format!("{identity}.fsproj"),
                "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net10.0</TargetFramework>\n  </PropertyGroup>\n  <ItemGroup>\n    <Compile Include=\"Library.fs\" />\n  </ItemGroup>\n</Project>\n"
                    .to_owned(),
            ),
            (
                "Library.fs".to_owned(),
                "module Hello\n\nlet greet name = \"hello \" + name\n".to_owned(),
            ),
        ],
        _ => vec![
            (
                "hello.cc".to_owned(),
                "#include \"hello.h\"\n\nstd::string Hello(const std::string& name) {\n  return \"hello \" + name;\n}\n"
                    .to_owned(),
            ),
            (
                "hello.h".to_owned(),
                "#pragma once\n\n#include <string>\n\nstd::string Hello(const std::string& name);\n"
                    .to_owned(),
            ),
        ],
    };
    Ok(files)
}

pub fn apply_new(root: &Path, language: &str, name: &str) -> Result<Vec<String>, AdoptError> {
    let files = plan_new_files(language, name)?;
    let mut staged = Vec::with_capacity(files.len());
    for file in &files {
        staged.push(super::scaffold_dest_within_root(root, &file.path)?);
    }
    let mut written = Vec::new();
    let mut refused = Vec::new();
    for (file, dest) in files.iter().zip(staged.iter()) {
        if dest.exists() {
            refused.push(format!("refused:{}", file.path));
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
                parent: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        dx_atomic_fs::write_atomic(dest, file.content.as_ref()).map_err(|e| {
            AdoptError::WriteFile {
                path: dest.display().to_string(),
                detail: e.to_string(),
            }
        })?;
        written.push(file.path.clone());
    }
    written.push("---".to_owned());
    written.extend(refused);
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::super::DX_VERSION;
    use super::*;

    #[test]
    fn new_aliases_normalize_to_canonical_templates() {
        assert_eq!(normalize_new_language("rust"), Some("rust"));
        assert_eq!(normalize_new_language("c"), Some("cpp"));
        assert_eq!(normalize_new_language("cc"), Some("cpp"));
        assert_eq!(normalize_new_language("cpp"), Some("cpp"));
        assert_eq!(normalize_new_language("c#"), Some("csharp"));
        assert_eq!(normalize_new_language("f#"), Some("fsharp"));
        assert_eq!(normalize_new_language("ruby"), None);
        assert!(new_is_known_language("go"));
        assert!(new_is_known_language("rust-web"));
        assert!(!new_is_known_language("swift"));
        assert_eq!(SUPPORTED_NEW_LANGUAGES.len(), 14);
    }

    #[test]
    fn every_supported_language_is_accepted_and_gets_its_own_template() {
        for language in SUPPORTED_NEW_LANGUAGES {
            let canonical = normalize_new_language(language)
                .unwrap_or_else(|| panic!("{language} is listed but refused"));
            assert!(
                SUPPORTED_NEW_LANGUAGES.contains(&canonical),
                "{language} normalizes to {canonical}, which is not a supported language"
            );
        }
        for (alias, canonical) in NEW_LANGUAGE_ALIASES {
            assert_eq!(normalize_new_language(alias), Some(*canonical));
            assert!(SUPPORTED_NEW_LANGUAGES.contains(canonical));
        }
        let mut seen: Vec<(String, Vec<String>)> = Vec::new();
        for language in SUPPORTED_NEW_LANGUAGES {
            let canonical = normalize_new_language(language).expect("canonical");
            if seen.iter().any(|(name, _)| name == canonical) {
                continue;
            }
            let files = plan_new_files(language, "demo")
                .expect("plans")
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>();
            assert!(
                !seen.iter().any(|(_, paths)| paths == &files),
                "{canonical} shares its template with another language"
            );
            seen.push((canonical.to_owned(), files));
        }
    }

    #[test]
    fn new_plans_init_wiring_plus_language_sources() {
        for lang in SUPPORTED_NEW_LANGUAGES {
            let files = plan_new_files(lang, "demo").expect("plans");
            assert!(
                files.iter().any(|f| f.path == "demo/.dx/version"),
                "{lang} carries init wiring"
            );
            assert!(
                files.iter().any(|f| f.path == "demo/.vscode/settings.json"),
                "{lang} carries editor wiring"
            );
        }
        let rust = plan_new_files("rust", "demo").expect("rust");
        assert!(rust.iter().any(|f| f.path == "demo/Cargo.toml"));
        assert!(rust.iter().any(|f| f.path == "demo/src/main.rs"));
        let go = plan_new_files("go", "demo").expect("go");
        assert!(go.iter().any(|f| f.path == "demo/go.mod"));
        let java = plan_new_files("java", "demo").expect("java");
        assert!(java.iter().any(|f| f.path == "demo/Hello.java"));
        let cpp = plan_new_files("c", "demo").expect("c alias");
        assert!(cpp.iter().any(|f| f.path == "demo/hello.cc"));
        let version = rust
            .iter()
            .find(|f| f.path == "demo/.dx/version")
            .expect("version");
        assert_eq!(version.content, format!("{DX_VERSION}\n"));
        assert!(plan_new_files("ruby", "demo").is_err());
        assert_eq!(
            plan_new_files("ruby", "demo").unwrap_err().to_string(),
            "unknown language for dx new: ruby (want one of rust, python, javascript, typescript, go, java, kotlin, scala, csharp, fsharp, c, cc, cpp, rust-web)"
        );
    }

    #[test]
    fn rust_web_template_carries_shared_native_and_browser_files() {
        let files = plan_new_files("rust-web", "demo").expect("plans");
        let paths = files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        for wanted in [
            "demo/Cargo.toml",
            "demo/src/lib.rs",
            "demo/src/main.rs",
            "demo/BUILD.bazel",
            "demo/MODULE.bazel",
            "demo/.bazelversion",
            "demo/README.md",
            "demo/.dx/version",
        ] {
            assert!(paths.contains(&wanted.to_owned()), "{paths:?}");
        }
        let main = files
            .iter()
            .find(|f| f.path == "demo/src/main.rs")
            .expect("main");
        assert!(main.content.contains("demo::greeting"), "{main:?}");
        assert!(!main.content.contains("hello world"), "{main:?}");
        let cargo = files
            .iter()
            .find(|f| f.path == "demo/Cargo.toml")
            .expect("cargo");
        assert_eq!(
            cargo.content,
            "[dependencies]\nwasm-bindgen = \"=0.2.121\"\n\n[package]\nedition = \"2021\"\nname = \"demo\"\nversion = \"0.1.0\"\n"
        );
        let build = files
            .iter()
            .find(|f| f.path == "demo/BUILD.bazel")
            .expect("build");
        for wanted in [
            "rust_library(",
            "rust_binary(",
            "rust_test(",
            "rustfmt_test(",
            "rust_clippy_test(",
            "rust_wasm_bindgen(",
            "rust_wasm_bindgen_test(",
            "name = \"demo\"",
            "name = \"demo_bin\"",
            "name = \"demo_web\"",
            "name = \"demo_browser_test\"",
            ":demo_bin_upstream",
            "@rules_rust_wasm_bindgen//3rdparty:wasm_bindgen",
            "tags = [\"manual\"]",
        ] {
            assert!(build.content.contains(wanted), "{wanted}");
        }
        let module = files
            .iter()
            .find(|f| f.path == "demo/MODULE.bazel")
            .expect("module");
        assert!(
            module.content.contains("module(name = \"demo\")"),
            "{module:?}"
        );
        assert!(
            module
                .content
                .contains("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")"),
            "{module:?}"
        );
        assert!(
            module
                .content
                .contains("bazel_dep(name = \"rules_rust_wasm_bindgen\", version = \"0.74.0\")"),
            "{module:?}"
        );
        assert!(
            module
                .content
                .contains("bazel_dep(name = \"platforms\", version = \"1.1.0\")"),
            "{module:?}"
        );
        let version = files
            .iter()
            .find(|f| f.path == "demo/.bazelversion")
            .expect("bazelversion");
        assert_eq!(version.content, "9.2.0\n");
        let readme = files
            .iter()
            .find(|f| f.path == "demo/README.md")
            .expect("readme");
        assert!(readme.content.contains("bazel test //..."), "{readme:?}");
        assert!(readme.content.contains("demo_browser_test"), "{readme:?}");
    }

    #[test]
    fn rust_web_template_folds_hyphenated_identities_to_crate_idents() {
        let files = plan_new_files("rust-web", "teams/My App").expect("plans");
        let cargo = files
            .iter()
            .find(|f| f.path == "teams/My App/Cargo.toml")
            .expect("cargo");
        assert!(cargo.content.contains("name = \"my-app\""), "{cargo:?}");
        let main = files
            .iter()
            .find(|f| f.path == "teams/My App/src/main.rs")
            .expect("main");
        assert!(main.content.contains("my_app::greeting"), "{main:?}");
        let build = files
            .iter()
            .find(|f| f.path == "teams/My App/BUILD.bazel")
            .expect("build");
        assert!(
            build.content.contains("crate_name = \"my_app\""),
            "{build:?}"
        );
        assert!(plan_new_files("rust-web", "+").is_err());
    }

    #[test]
    fn default_name_and_parent_collisions_are_explicit() {
        let files = plan_new_files("rust", "").expect("default name");
        assert!(files
            .iter()
            .all(|file| file.path.starts_with("my_project/")));
        let scratch = dx_test_scratch::scratch("new-parent-collision-");
        std::fs::write(scratch.path().join("demo"), "foreign").expect("collision");
        assert!(matches!(
            apply_new(scratch.path(), "rust", "demo"),
            Err(AdoptError::CreateParent { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("demo")).expect("foreign"),
            "foreign"
        );
    }

    #[test]
    fn new_templates_are_stdlib_only_for_generate() {
        let rust = plan_new_files("rust", "demo").expect("rust");
        let cargo = rust
            .iter()
            .find(|f| f.path == "demo/Cargo.toml")
            .expect("cargo");
        assert!(!cargo.content.contains("anyhow"));
        let python = plan_new_files("python", "demo").expect("python");
        let pyproject = python
            .iter()
            .find(|f| f.path == "demo/pyproject.toml")
            .expect("pyproject");
        assert!(pyproject.content.contains("dependencies = []"));
        let js = plan_new_files("javascript", "demo").expect("js");
        let package = js
            .iter()
            .find(|f| f.path == "demo/package.json")
            .expect("package");
        assert!(!package.content.contains("jest"));
    }

    #[test]
    fn apply_new_writes_absent_only_and_refuses_existing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = apply_new(&root, "go", "demo").expect("new");
        assert!(first.iter().any(|p| p == "demo/go.mod"));
        assert!(root.join("demo/go.mod").exists());
        assert!(root.join("demo/.dx/version").exists());
        let second = apply_new(&root, "go", "demo").expect("new again");
        assert!(second.iter().any(|p| p == "refused:demo/go.mod"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn destinations_reject_escape_and_hostile_shapes() {
        for hostile in [
            "../evil",
            "a/../../evil",
            "/tmp/absolute",
            "C:/absolute",
            "C:\\absolute",
            "a\\b",
            "a//b",
            "trailing/",
            ".",
            "..",
            "a/.",
            "a/..",
            "with\nnewline",
            "with\"quote",
            "with*star",
            "with?mark",
            "with|pipe",
            "with<angle>",
            "with:colon",
            "endswith ",
            "endswith.",
        ] {
            assert!(
                validate_new_destination(hostile).is_err(),
                "{hostile:?} must not be a destination"
            );
            assert!(
                plan_new_files("rust", hostile).is_err(),
                "{hostile:?} must not plan"
            );
        }
        assert_eq!(validate_new_destination("").expect("default"), "my_project");
        assert_eq!(validate_new_destination("demo").expect("plain"), "demo");
        assert_eq!(
            validate_new_destination("teams/demo").expect("nested"),
            "teams/demo"
        );
    }

    #[test]
    fn destinations_reject_reserved_device_names() {
        for hostile in ["CON", "nul", "com1", "LPT9", "aux.txt", "con.json"] {
            assert!(
                validate_new_destination(hostile).is_err(),
                "{hostile:?} is reserved"
            );
        }
        assert!(validate_new_destination("console").is_ok());
        assert!(validate_new_destination("contact").is_ok());
    }

    #[test]
    fn spaces_and_unicode_destinations_fold_to_valid_identities() {
        assert_eq!(
            derive_new_identity("rust", "My App").expect("fold"),
            "my-app"
        );
        assert_eq!(derive_new_identity("rust", "café").expect("fold"), "caf");
        assert_eq!(
            derive_new_identity("go", "teams/Demo").expect("nested stem"),
            "demo"
        );
        let files = plan_new_files("rust", "My App").expect("plans");
        assert!(files.iter().any(|f| f.path == "My App/Cargo.toml"));
        let cargo = files
            .iter()
            .find(|f| f.path == "My App/Cargo.toml")
            .expect("cargo");
        let parsed: toml::Table = cargo.content.parse().expect("valid TOML");
        assert_eq!(parsed["package"]["name"].as_str(), Some("my-app"));
        let ci = files
            .iter()
            .find(|f| f.path == "My App/.github/workflows/ci.yml")
            .expect("ci wiring");
        assert!(ci.content.contains("uses: rules_dx/.github/workflows/"));
        assert!(!ci.content.contains("uses: my-app/"));
        let nested = plan_new_files("javascript", "teams/My App").expect("plans");
        assert!(nested.iter().any(|f| f.path == "teams/My App/package.json"));
    }

    #[test]
    fn derivation_failures_name_the_ecosystem_rule() {
        let error = derive_new_identity("rust", "+").unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid package identity for dx new rust: \"\": cargo crate names use [A-Za-z0-9_-] only"
        );
        assert!(plan_new_files("rust", "+").is_err());
    }

    #[test]
    fn scaffold_manifests_keep_their_exact_shape() {
        let rust = plan_new_files("rust", "demo").expect("rust");
        let cargo = rust
            .iter()
            .find(|f| f.path == "demo/Cargo.toml")
            .expect("cargo");
        assert_eq!(
            cargo.content,
            "[package]\nedition = \"2021\"\nname = \"demo\"\nversion = \"0.1.0\"\n"
        );
        let python = plan_new_files("python", "demo").expect("python");
        let pyproject = python
            .iter()
            .find(|f| f.path == "demo/pyproject.toml")
            .expect("pyproject");
        assert_eq!(
            pyproject.content,
            "[project]\ndependencies = []\nname = \"demo\"\nrequires-python = \">=3.12\"\nversion = \"0.1.0\"\n"
        );
        let js = plan_new_files("javascript", "demo").expect("js");
        let package = js
            .iter()
            .find(|f| f.path == "demo/package.json")
            .expect("package");
        assert_eq!(
            package.content,
            "{\n  \"name\": \"demo\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n"
        );
        let go = plan_new_files("go", "demo").expect("go");
        let gomod = go.iter().find(|f| f.path == "demo/go.mod").expect("go.mod");
        assert_eq!(gomod.content, "module demo\n\ngo 1.26\n");
    }

    #[test]
    fn scaffold_manifests_parse_with_real_libraries() {
        for language in SUPPORTED_NEW_LANGUAGES {
            let files = plan_new_files(language, "demo").expect("plans");
            for file in &files {
                if file.path.ends_with("Cargo.toml") || file.path.ends_with("pyproject.toml") {
                    file.content.parse::<toml::Table>().expect("valid TOML");
                }
                if file.path.ends_with("package.json")
                    || file.path.ends_with("tsconfig.json")
                    || file.path.ends_with(".vscode/settings.json")
                    || file.path.ends_with(".vscode/extensions.json")
                {
                    serde_json::from_str::<serde_json::Value>(&file.content).expect("valid JSON");
                }
            }
        }
        let java = plan_new_files("java", "demo").expect("java");
        let pom = java.iter().find(|f| f.path == "demo/pom.xml").expect("pom");
        assert!(pom.content.contains("<artifactId>demo</artifactId>"));
        let scala = plan_new_files("scala", "demo").expect("scala");
        let sbt = scala
            .iter()
            .find(|f| f.path == "demo/build.sbt")
            .expect("sbt");
        assert!(sbt.content.contains("name := \"demo\""));
        let csharp = plan_new_files("csharp", "demo").expect("csharp");
        assert!(csharp.iter().any(|f| f.path == "demo/demo.csproj"));
    }

    #[test]
    fn manifest_escapers_neutralize_breakout_characters() {
        assert_eq!(escape_json_string("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape_json_string("a\nb\rc\td"), "a\\nb\\rc\\td");
        assert_eq!(escape_json_string("a\u{0}b"), "a\\u0000b");
        assert_eq!(escape_json_string("plain"), "plain");
        assert_eq!(escape_xml("a&b<c>\"d'e"), "a&amp;b&lt;c&gt;&quot;d&apos;e");
        assert_eq!(escape_sbt_string("a\"b\\c"), "a\\\"b\\\\c");
    }

    #[test]
    fn apply_new_writes_nothing_for_rejected_names() {
        for hostile in ["../evil", "/tmp/absolute", "with\nnewline", "a\\b", "+"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-new-reject-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            assert!(
                apply_new(&root, "rust", hostile).is_err(),
                "{hostile:?} must fail"
            );
            let mut entries: Vec<String> = Vec::new();
            for entry in std::fs::read_dir(&root).expect("read") {
                entries.push(
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            assert_eq!(entries, vec!["sentinel".to_owned()], "{hostile:?}");
            scratch.close().expect("cleanup");
        }
    }

    #[test]
    fn scaffold_dest_within_root_refuses_non_normal_paths() {
        let root = std::path::Path::new("/root");
        assert!(super::super::scaffold_dest_within_root(root, "demo/a").is_ok());
        for hostile in ["../evil", "/absolute", "", "a/../../b"] {
            assert!(
                super::super::scaffold_dest_within_root(root, hostile).is_err(),
                "{hostile:?} escapes"
            );
        }
    }
}
