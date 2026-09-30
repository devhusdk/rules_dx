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
    let project = if name.is_empty() {
        default_new_name()
    } else {
        name
    };
    let mut files = Vec::new();
    for file in super::plan_init_files(project) {
        files.push(ScaffoldFile {
            path: format!("{project}/{}", file.path),
            content: file.content,
        });
    }
    for (suffix, content) in new_language_files(canonical, project) {
        files.push(ScaffoldFile {
            path: format!("{project}/{suffix}"),
            content,
        });
    }
    Ok(files)
}

fn new_language_files(canonical: &str, project: &str) -> Vec<(String, String)> {
    match canonical {
        "rust" => vec![
            (
                "Cargo.toml".to_owned(),
                format!(
                    "[package]\nname = \"{project}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
                ),
            ),
            (
                "src/main.rs".to_owned(),
                "fn main() {\n    println!(\"hello world\");\n}\n".to_owned(),
            ),
        ],
        "python" => vec![
            (
                "pyproject.toml".to_owned(),
                format!(
                    "[project]\nname = \"{project}\"\nversion = \"0.1.0\"\nrequires-python = \">=3.12\"\ndependencies = []\n"
                ),
            ),
            (
                "hello.py".to_owned(),
                "\"\"\"Greeting helper with no dependencies.\"\"\"\n\n\ndef greet(name):\n    \"\"\"Return a greeting for name.\"\"\"\n    return f\"Hello, {{name}}!\"\n".to_owned(),
            ),
        ],
        "javascript" => vec![
            (
                "package.json".to_owned(),
                format!(
                    "{{\n  \"name\": \"{project}\",\n  \"private\": true,\n  \"type\": \"module\"\n}}\n"
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
                    "{{\n  \"name\": \"{project}\",\n  \"private\": true,\n  \"type\": \"module\"\n}}\n"
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
                format!("module {project}\n\ngo 1.26\n"),
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
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>example.com</groupId>\n  <artifactId>{project}</artifactId>\n  <version>0.1.0</version>\n  <packaging>jar</packaging>\n</project>\n"
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
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>example.com</groupId>\n  <artifactId>{project}</artifactId>\n  <version>0.1.0</version>\n  <packaging>jar</packaging>\n</project>\n"
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
                    "ThisBuild / scalaVersion := \"2.13.18\"\nThisBuild / organization := \"example.com\"\nlazy val root = (project in file(\".\")).settings(name := \"{project}\")\n"
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
                format!("{project}.csproj"),
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
                format!("{project}.fsproj"),
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
    }
}

pub fn apply_new(root: &Path, language: &str, name: &str) -> Result<Vec<String>, AdoptError> {
    let mut written = Vec::new();
    let mut refused = Vec::new();
    for file in plan_new_files(language, name)? {
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
        written.push(file.path);
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
}
