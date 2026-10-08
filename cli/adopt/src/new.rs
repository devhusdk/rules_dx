use std::path::Path;

use dx_path::WorkspaceRelativePath;

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

const WINDOWS_RESERVED_STEMS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

fn destination_component_reason(component: &str) -> Option<String> {
    let stem = component.split('.').next().unwrap_or("");
    if WINDOWS_RESERVED_STEMS
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Some(format!("reserved Windows name {component:?}"));
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return Some(format!("trailing dot or space in {component:?}"));
    }
    None
}

pub fn validate_new_destination(name: &str) -> Result<String, AdoptError> {
    if name.is_empty() {
        return Ok(default_new_name().to_owned());
    }
    let canonical =
        WorkspaceRelativePath::new(name).map_err(|problem| AdoptError::NewInvalidDestination {
            name: name.to_owned(),
            reason: problem.reason().to_owned(),
        })?;
    for component in canonical.as_str().split('/') {
        if let Some(reason) = destination_component_reason(component) {
            return Err(AdoptError::NewInvalidDestination {
                name: name.to_owned(),
                reason,
            });
        }
    }
    Ok(canonical.as_str().to_owned())
}

pub fn derive_new_package(destination: &str) -> String {
    let stem = destination.rsplit('/').next().unwrap_or("");
    let mut package = String::with_capacity(stem.len());
    let mut underscore = false;
    for ch in stem.to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            package.push(ch);
            underscore = false;
        } else if !underscore {
            package.push('_');
            underscore = true;
        }
    }
    let trimmed = package.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        return default_new_name().to_owned();
    }
    if trimmed.starts_with(|ch: char| ch.is_ascii_digit()) {
        return format!("app_{trimmed}");
    }
    trimmed
}

fn xml_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn sbt_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn render_cargo_manifest(package: &str) -> Result<String, AdoptError> {
    let mut fields = toml::Table::new();
    fields.insert("name".to_owned(), toml::Value::String(package.to_owned()));
    fields.insert(
        "version".to_owned(),
        toml::Value::String("0.1.0".to_owned()),
    );
    fields.insert("edition".to_owned(), toml::Value::String("2021".to_owned()));
    let mut root = toml::Table::new();
    root.insert("package".to_owned(), toml::Value::Table(fields));
    toml::to_string(&root).map_err(|error| AdoptError::RenderManifest {
        what: "Cargo.toml".to_owned(),
        detail: error.to_string(),
    })
}

fn render_pyproject_manifest(package: &str) -> Result<String, AdoptError> {
    let mut fields = toml::Table::new();
    fields.insert("name".to_owned(), toml::Value::String(package.to_owned()));
    fields.insert(
        "version".to_owned(),
        toml::Value::String("0.1.0".to_owned()),
    );
    fields.insert(
        "requires-python".to_owned(),
        toml::Value::String(">=3.12".to_owned()),
    );
    fields.insert("dependencies".to_owned(), toml::Value::Array(Vec::new()));
    let mut root = toml::Table::new();
    root.insert("project".to_owned(), toml::Value::Table(fields));
    toml::to_string(&root).map_err(|error| AdoptError::RenderManifest {
        what: "pyproject.toml".to_owned(),
        detail: error.to_string(),
    })
}

fn render_package_json(package: &str) -> Result<String, AdoptError> {
    let document = serde_json::json!({
        "name": package,
        "private": true,
        "type": "module",
    });
    serde_json::to_string_pretty(&document)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| AdoptError::RenderManifest {
            what: "package.json".to_owned(),
            detail: error.to_string(),
        })
}

pub fn plan_new_files(language: &str, name: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let canonical =
        normalize_new_language(language).ok_or_else(|| AdoptError::NewUnknownLanguage {
            language: language.to_owned(),
        })?;
    let project = validate_new_destination(name)?;
    let package = derive_new_package(&project);
    let mut files = Vec::new();
    for file in super::plan_init_files(&package)? {
        files.push(ScaffoldFile {
            path: format!("{project}/{}", file.path),
            content: file.content,
        });
    }
    for (suffix, content) in new_language_files(canonical, &package)? {
        files.push(ScaffoldFile {
            path: format!("{project}/{suffix}"),
            content,
        });
    }
    Ok(files)
}

fn new_language_files(canonical: &str, package: &str) -> Result<Vec<(String, String)>, AdoptError> {
    match canonical {
        "rust" => Ok(vec![
            ("Cargo.toml".to_owned(), render_cargo_manifest(package)?),
            (
                "src/main.rs".to_owned(),
                "fn main() {\n    println!(\"hello world\");\n}\n".to_owned(),
            ),
        ]),
        "python" => Ok(vec![
            (
                "pyproject.toml".to_owned(),
                render_pyproject_manifest(package)?,
            ),
            (
                "hello.py".to_owned(),
                "\"\"\"Greeting helper with no dependencies.\"\"\"\n\n\ndef greet(name):\n    \"\"\"Return a greeting for name.\"\"\"\n    return f\"Hello, {{name}}!\"\n".to_owned(),
            ),
        ]),
        "javascript" => Ok(vec![
            ("package.json".to_owned(), render_package_json(package)?),
            (
                "hello.js".to_owned(),
                "export function hello(name) {\n\treturn `hello ${name}`;\n}\n".to_owned(),
            ),
        ]),
        "typescript" => Ok(vec![
            ("package.json".to_owned(), render_package_json(package)?),
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
        ]),
        "go" => Ok(vec![
            (
                "go.mod".to_owned(),
                format!("module {package}\n\ngo 1.26\n"),
            ),
            (
                "hello.go".to_owned(),
                "package hello\n\n// Hello returns a greeting for name.\nfunc Hello(name string) string {\n\treturn \"hello \" + name\n}\n"
                    .to_owned(),
            ),
        ]),
        "java" => Ok(vec![
            (
                "pom.xml".to_owned(),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>example.com</groupId>\n  <artifactId>{}</artifactId>\n  <version>0.1.0</version>\n  <packaging>jar</packaging>\n</project>\n",
                    xml_escape(package)
                ),
            ),
            (
                "Hello.java".to_owned(),
                "package hello;\n\npublic class Hello {\n  public static String hello(String name) {\n    return \"hello \" + name;\n  }\n}\n"
                    .to_owned(),
            ),
        ]),
        "kotlin" => Ok(vec![
            (
                "pom.xml".to_owned(),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>example.com</groupId>\n  <artifactId>{}</artifactId>\n  <version>0.1.0</version>\n  <packaging>jar</packaging>\n</project>\n",
                    xml_escape(package)
                ),
            ),
            (
                "Hello.kt".to_owned(),
                "package hello\n\nobject Hello {\n  fun hello(name: String): String {\n    return \"hello \" + name\n  }\n}\n"
                    .to_owned(),
            ),
        ]),
        "scala" => Ok(vec![
            (
                "build.sbt".to_owned(),
                format!(
                    "ThisBuild / scalaVersion := \"2.13.18\"\nThisBuild / organization := \"example.com\"\nlazy val root = (project in file(\".\")).settings(name := \"{}\")\n",
                    sbt_escape(package)
                ),
            ),
            (
                "Hello.scala".to_owned(),
                "package hello\n\nobject Hello {\n  def hello(name: String): String = \"hello \" + name\n}\n"
                    .to_owned(),
            ),
        ]),
        "csharp" => Ok(vec![
            (
                format!("{package}.csproj"),
                "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net10.0</TargetFramework>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n</Project>\n"
                    .to_owned(),
            ),
            (
                "Hello.cs".to_owned(),
                "namespace Hello;\n\npublic static class Greeter\n{\n    public static string Greet(string name)\n    {\n        return \"hello \" + name;\n    }\n}\n"
                    .to_owned(),
            ),
        ]),
        "fsharp" => Ok(vec![
            (
                format!("{package}.fsproj"),
                "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net10.0</TargetFramework>\n  </PropertyGroup>\n  <ItemGroup>\n    <Compile Include=\"Library.fs\" />\n  </ItemGroup>\n</Project>\n"
                    .to_owned(),
            ),
            (
                "Library.fs".to_owned(),
                "module Hello\n\nlet greet name = \"hello \" + name\n".to_owned(),
            ),
        ]),
        _ => Ok(vec![
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
        ])
    }
}

pub fn apply_new(root: &Path, language: &str, name: &str) -> Result<Vec<String>, AdoptError> {
    let files = plan_new_files(language, name)?;
    super::preflight_scaffold_paths(root, &files)?;
    let mut written = Vec::new();
    let mut refused = Vec::new();
    for file in &files {
        let dest = root.join(&file.path);
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
        dx_atomic_fs::write_atomic(&dest, file.content.as_ref()).map_err(|e| {
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
        assert!(!new_is_known_language("swift"));
        assert_eq!(SUPPORTED_NEW_LANGUAGES.len(), 13);
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
            "unknown language for dx new: ruby (want one of rust, python, javascript, typescript, go, java, kotlin, scala, csharp, fsharp, c, cc, cpp)"
        );
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
            Err(AdoptError::ScaffoldBlocked { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("demo")).expect("foreign"),
            "foreign"
        );
        assert!(
            !scratch.path().join("demo/Cargo.toml").exists(),
            "preflight writes nothing when a parent is blocked"
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
    fn nested_destination_stays_inside_the_root() {
        let files = plan_new_files("rust", "a/b").expect("plans");
        assert!(files.iter().all(|file| file.path.starts_with("a/b/")));
        let scratch = dx_test_scratch::scratch("new-nested-");
        apply_new(scratch.path(), "rust", "a/b").expect("applies");
        assert!(scratch.path().join("a/b/Cargo.toml").exists());
        let blocked = dx_test_scratch::scratch("new-nested-blocked-");
        std::fs::write(blocked.path().join("a"), "foreign").expect("collision");
        assert!(matches!(
            apply_new(blocked.path(), "rust", "a/b"),
            Err(AdoptError::ScaffoldBlocked { .. })
        ));
        assert!(!blocked.path().join("a/b").exists());
    }

    #[test]
    fn derivation_maps_destinations_to_one_valid_package() {
        for (destination, package) in [
            ("demo", "demo"),
            ("my_project", "my_project"),
            ("my app", "my_app"),
            ("MyApp", "myapp"),
            ("my-app", "my_app"),
            ("my.app", "my_app"),
            ("2cool", "app_2cool"),
            ("_lead", "lead"),
            ("trail_", "trail"),
            ("a//b", "b"),
            ("nested/dir", "dir"),
            ("café", "caf"),
            ("日本語", "my_project"),
            ("a\"b", "a_b"),
            ("a'b", "a_b"),
            ("UPPER Mixed-Case.Name", "upper_mixed_case_name"),
        ] {
            assert_eq!(
                derive_new_package(destination),
                package,
                "destination: {destination:?}"
            );
        }
    }

    #[test]
    fn rejected_destinations_fail_before_any_write() {
        for name in [
            "../escape",
            "a/../../b",
            "..",
            "a/../b",
            "/absolute",
            "C:/windows",
            "C:relative",
            "a/./b",
            "a\\back",
            "with\nnewline",
            "with\0nul",
            "con",
            "CON",
            "prn.txt",
            "Aux",
            "nul",
            "com1",
            "COM9",
            "lpt1",
            "LPT9",
            "nested/con/here",
            "trail ",
            "trail.",
            "nested/trail. ",
        ] {
            let error = plan_new_files("rust", name).expect_err("must reject");
            assert!(
                matches!(error, AdoptError::NewInvalidDestination { .. }),
                "name {name:?}: {error}"
            );
            let scratch = dx_test_scratch::scratch("new-reject-");
            assert!(
                matches!(
                    apply_new(scratch.path(), "rust", name),
                    Err(AdoptError::NewInvalidDestination { .. })
                ),
                "name {name:?}"
            );
            let entries: Vec<_> = std::fs::read_dir(scratch.path())
                .expect("list")
                .collect::<Result<_, _>>()
                .expect("entries");
            assert!(entries.is_empty(), "name {name:?} wrote files");
        }
    }

    #[test]
    fn quoted_destination_keeps_a_valid_manifest() {
        let files = plan_new_files("rust", "we\"ird").expect("plans");
        assert!(
            files.iter().all(|file| file.path.starts_with("we\"ird/")),
            "{files:?}"
        );
        let cargo = files
            .iter()
            .find(|file| file.path == "we\"ird/Cargo.toml")
            .expect("cargo");
        let parsed: toml::Table = cargo.content.parse().expect("parses");
        assert_eq!(
            parsed["package"]["name"].as_str(),
            Some("we_ird"),
            "{}",
            cargo.content
        );
    }

    #[test]
    fn spaces_and_unicode_destinations_derive_parseable_manifests() {
        for (language, manifest, name_key) in [
            ("rust", "Cargo.toml", "package.name"),
            ("python", "pyproject.toml", "project.name"),
            ("javascript", "package.json", "name"),
            ("typescript", "package.json", "name"),
            ("go", "go.mod", ""),
            ("java", "pom.xml", "artifactId"),
            ("kotlin", "pom.xml", "artifactId"),
            ("scala", "build.sbt", "name"),
            ("csharp", "my_app.csproj", ""),
            ("fsharp", "my_app.fsproj", ""),
            ("c", "hello.h", ""),
            ("cpp", "hello.h", ""),
        ] {
            let files = plan_new_files(language, "my app").expect("plans");
            assert!(
                files.iter().all(|file| file.path.starts_with("my app/")),
                "{language}: {files:?}"
            );
            if manifest == "go.mod" {
                let go_mod = files
                    .iter()
                    .find(|file| file.path == "my app/go.mod")
                    .expect("go.mod");
                assert_eq!(go_mod.content, "module my_app\n\ngo 1.26\n");
                continue;
            }
            if manifest.ends_with(".csproj") || manifest.ends_with(".fsproj") {
                assert!(
                    files
                        .iter()
                        .any(|file| file.path == format!("my app/{manifest}")),
                    "{language}: {files:?}"
                );
                continue;
            }
            if manifest == "hello.h" {
                assert!(
                    files.iter().any(|file| file.path == "my app/hello.h"),
                    "{language}: {files:?}"
                );
                continue;
            }
            let entry = files
                .iter()
                .find(|file| file.path == format!("my app/{manifest}"))
                .unwrap_or_else(|| panic!("{language} {manifest}"));
            if manifest.ends_with(".toml") {
                let parsed: toml::Table = entry.content.parse().expect("parses");
                let mut parts = name_key.split('.');
                let section = parts.next().expect("section");
                let key = parts.next().expect("key");
                let table = parsed
                    .get(section)
                    .and_then(toml::Value::as_table)
                    .unwrap_or_else(|| panic!("{language} section {section}"));
                assert_eq!(
                    table.get(key).and_then(toml::Value::as_str),
                    Some("my_app"),
                    "{language}: {}",
                    entry.content
                );
            } else if manifest.ends_with(".json") {
                let parsed: serde_json::Value =
                    serde_json::from_str(&entry.content).expect("parses");
                assert_eq!(parsed["name"], serde_json::Value::from("my_app"));
            } else {
                assert!(
                    entry.content.contains("my_app"),
                    "{language}: {}",
                    entry.content
                );
            }
        }
    }

    #[test]
    fn cargo_and_pyproject_render_through_the_toml_serializer() {
        let rust = plan_new_files("rust", "demo").expect("rust");
        let cargo = rust
            .iter()
            .find(|file| file.path == "demo/Cargo.toml")
            .expect("cargo");
        assert_eq!(
            cargo.content,
            "[package]\nedition = \"2021\"\nname = \"demo\"\nversion = \"0.1.0\"\n"
        );
        let parsed: toml::Table = cargo.content.parse().expect("parses");
        assert_eq!(parsed["package"]["name"].as_str(), Some("demo"));
        let python = plan_new_files("python", "demo").expect("python");
        let pyproject = python
            .iter()
            .find(|file| file.path == "demo/pyproject.toml")
            .expect("pyproject");
        assert_eq!(
            pyproject.content,
            "[project]\ndependencies = []\nname = \"demo\"\nrequires-python = \">=3.12\"\nversion = \"0.1.0\"\n"
        );
        let parsed: toml::Table = pyproject.content.parse().expect("parses");
        assert_eq!(parsed["project"]["name"].as_str(), Some("demo"));
    }

    #[test]
    fn package_json_renders_through_the_json_serializer() {
        let js = plan_new_files("javascript", "my app").expect("js");
        let package = js
            .iter()
            .find(|file| file.path == "my app/package.json")
            .expect("package");
        assert_eq!(
            package.content,
            "{\n  \"name\": \"my_app\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n"
        );
        let parsed: serde_json::Value = serde_json::from_str(&package.content).expect("parses");
        assert_eq!(parsed["name"], serde_json::Value::from("my_app"));
    }

    #[test]
    fn xml_and_sbt_escapers_neutralize_markup() {
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert_eq!(
            xml_escape("<a>\"b\"</a>"),
            "&lt;a&gt;&quot;b&quot;&lt;/a&gt;"
        );
        assert_eq!(xml_escape("o'clock"), "o&apos;clock");
        assert_eq!(sbt_escape("a\"b\\c"), "a\\\"b\\\\c");
    }

    #[test]
    fn error_strings_name_the_rejected_input() {
        assert_eq!(
            validate_new_destination("../escape")
                .unwrap_err()
                .to_string(),
            "invalid destination for dx new: \"../escape\": path must have no '..' component"
        );
        assert_eq!(
            validate_new_destination("con").unwrap_err().to_string(),
            "invalid destination for dx new: \"con\": reserved Windows name \"con\""
        );
    }
}
