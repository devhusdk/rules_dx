use std::path::Path;

use super::AdoptError;
use crate::scaffold::ScaffoldFile;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdentityRule {
    Cargo,
    Dotted,
    Npm,
    Go,
    Maven,
    Nuget,
    BazelModule,
}

impl IdentityRule {
    pub fn doc_name(self) -> &'static str {
        match self {
            IdentityRule::Cargo => "Cargo names",
            IdentityRule::Dotted => "dotted names",
            IdentityRule::Npm => "npm names",
            IdentityRule::Go => "module paths",
            IdentityRule::Maven => "Maven artifact ids",
            IdentityRule::Nuget => "NuGet ids",
            IdentityRule::BazelModule => "Bazel module names",
        }
    }

    fn validate(self, identity: &str) -> Result<(), &'static str> {
        match self {
            IdentityRule::Cargo => dx_identity::validate_cargo(identity),
            IdentityRule::Dotted => dx_identity::validate_dotted(
                identity,
                "python project names use [A-Za-z0-9_.-] only",
            ),
            IdentityRule::Npm => dx_identity::validate_npm(identity),
            IdentityRule::Go => dx_identity::validate_go(identity),
            IdentityRule::Maven => {
                dx_identity::validate_dotted(identity, "maven artifact ids use [A-Za-z0-9_.-] only")
            }
            IdentityRule::Nuget => dx_identity::validate_nuget(identity),
            IdentityRule::BazelModule => validate_bazel_module_name(identity),
        }
    }
}

pub struct NewLanguage {
    pub name: &'static str,
    pub identity: IdentityRule,
}

pub const NEW_LANGUAGES: &[NewLanguage] = &[
    NewLanguage {
        name: "rust",
        identity: IdentityRule::Cargo,
    },
    NewLanguage {
        name: "python",
        identity: IdentityRule::Dotted,
    },
    NewLanguage {
        name: "javascript",
        identity: IdentityRule::Npm,
    },
    NewLanguage {
        name: "typescript",
        identity: IdentityRule::Npm,
    },
    NewLanguage {
        name: "go",
        identity: IdentityRule::Go,
    },
    NewLanguage {
        name: "java",
        identity: IdentityRule::Maven,
    },
    NewLanguage {
        name: "kotlin",
        identity: IdentityRule::Maven,
    },
    NewLanguage {
        name: "scala",
        identity: IdentityRule::Maven,
    },
    NewLanguage {
        name: "csharp",
        identity: IdentityRule::Nuget,
    },
    NewLanguage {
        name: "fsharp",
        identity: IdentityRule::Nuget,
    },
    NewLanguage {
        name: "c",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "cpp",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "rust-web",
        identity: IdentityRule::Cargo,
    },
    NewLanguage {
        name: "rust-android",
        identity: IdentityRule::Cargo,
    },
    NewLanguage {
        name: "ruby",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "powershell",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "astro",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "svelte",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "vue",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "mdx",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "zig",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "elixir",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "erlang",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "ocaml",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "haskell",
        identity: IdentityRule::BazelModule,
    },
    NewLanguage {
        name: "cuda",
        identity: IdentityRule::BazelModule,
    },
];

const NEW_LANGUAGE_COUNT: usize = NEW_LANGUAGES.len();

const NEW_LANGUAGE_NAMES: [&str; NEW_LANGUAGE_COUNT] = {
    let mut names = [""; NEW_LANGUAGE_COUNT];
    let mut index = 0;
    while index < NEW_LANGUAGE_COUNT {
        names[index] = NEW_LANGUAGES[index].name;
        index += 1;
    }
    names
};

pub const SUPPORTED_NEW_LANGUAGES: &[&str] = &NEW_LANGUAGE_NAMES;

pub fn new_language_identity_rule(language: &str) -> Option<IdentityRule> {
    NEW_LANGUAGES
        .iter()
        .find(|entry| entry.name == language)
        .map(|entry| entry.identity)
}

pub const ASPECT_RULES_JS_VERSION: &str = "3.5.2";
pub const ASTRO_COMPILER_VERSION: &str = "4.0.0";
pub const SVELTE_VERSION: &str = "5.57.0";
pub const VUE_COMPILER_VERSION: &str = "3.5.42";
pub const MDX_VERSION: &str = "3.1.1";

pub const RUST_WEB_EDITION: &str = "2021";
pub const RUST_WEB_WASM_BINDGEN_CRATE_VERSION: &str = "0.2.121";
pub const RUST_WEB_WASM_BINDGEN_TEST_CRATE_VERSION: &str = "0.3.71";
pub const RULES_RUST_WASM_BINDGEN_VERSION: &str = "0.74.0";
pub const RUST_WEB_PLATFORMS_VERSION: &str = "1.1.0";
pub const RUST_WEB_BAZEL_VERSION: &str = "9.2.0";

/// The `minSdkVersion` the rust-android template writes; equal to rules_dx's API floor.
pub const RUST_ANDROID_API_LEVEL: u32 = 31;

/// The adb serial the rust-android launchers name: the first local emulator.
pub const RUST_ANDROID_SERIAL: &str = "emulator-5554";

const REMOVED_NEW_LANGUAGES: &[(&str, &str)] = &[
    ("cc", "cpp"),
    ("c#", "csharp"),
    ("c-sharp", "csharp"),
    ("f#", "fsharp"),
    ("f-sharp", "fsharp"),
];

pub fn new_language_name_list() -> String {
    SUPPORTED_NEW_LANGUAGES.join(", ")
}

pub fn removed_new_language(language: &str) -> Option<&'static str> {
    REMOVED_NEW_LANGUAGES
        .iter()
        .find(|(spelling, _)| *spelling == language)
        .map(|(_, canonical)| *canonical)
}

pub fn normalize_new_language(language: &str) -> Option<&'static str> {
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
    if let Some(canonical) = removed_new_language(language) {
        return Err(AdoptError::NewRemovedLanguage {
            language: language.to_owned(),
            canonical: canonical.to_owned(),
        });
    }
    let canonical =
        normalize_new_language(language).ok_or_else(|| AdoptError::NewUnknownLanguage {
            language: language.to_owned(),
        })?;
    let destination = validate_new_destination(name)?;
    let identity = derive_new_identity(canonical, &destination)?;
    let language_files = new_language_files(canonical, &identity)?;
    let mut files = Vec::new();
    for file in super::plan_init_files(&identity, super::InitParts::ALL)? {
        if language_files
            .iter()
            .any(|(suffix, _)| *suffix == file.path)
        {
            continue;
        }
        files.push(ScaffoldFile {
            path: format!("{destination}/{}", file.path),
            content: file.content,
        });
    }
    for (suffix, content) in language_files {
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
    let check =
        new_language_identity_rule(canonical).map_or(Ok(()), |rule| rule.validate(&identity));
    check.map_err(|reason| AdoptError::NewInvalidIdentity {
        language: canonical.to_owned(),
        name: identity.clone(),
        reason: reason.to_owned(),
    })?;
    Ok(identity)
}

fn validate_bazel_module_name(name: &str) -> Result<(), &'static str> {
    const REASON: &str =
        "bazel module names start with a letter, end with a letter or digit, and use [a-z0-9._-] only";
    let bytes = name.as_bytes();
    let (Some(first), Some(last)) = (bytes.first(), bytes.last()) else {
        return Err(REASON);
    };
    if !first.is_ascii_lowercase() || !(last.is_ascii_lowercase() || last.is_ascii_digit()) {
        return Err(REASON);
    }
    if bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(REASON)
    }
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
    let mut dev_dependencies = toml::Table::new();
    dev_dependencies.insert(
        "wasm-bindgen-test".to_owned(),
        toml::Value::String(format!("={RUST_WEB_WASM_BINDGEN_TEST_CRATE_VERSION}")),
    );
    let mut root = toml::Table::new();
    root.insert("package".to_owned(), toml::Value::Table(package));
    root.insert("dependencies".to_owned(), toml::Value::Table(dependencies));
    root.insert(
        "dev-dependencies".to_owned(),
        toml::Value::Table(dev_dependencies),
    );
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
        "fn main() {{\n    println!(\"{{}}\", {ident}::greeting(\"native\"));\n}}\n\n#[cfg(test)]\nmod browser_tests {{\n    use wasm_bindgen_test::*;\n\n    #[cfg(target_arch = \"wasm32\")]\n    #[wasm_bindgen_test]\n    fn greeting_roundtrip() {{\n        assert_eq!({ident}::greeting(\"browser\"), \"Hello from browser!\");\n    }}\n\n    #[cfg(target_arch = \"wasm32\")]\n    #[wasm_bindgen_test]\n    fn add_roundtrip() {{\n        assert_eq!({ident}::add(40, 2), 42);\n    }}\n}}\n",
        ident = rust_web_crate_ident(identity)
    )
}

fn rust_web_build_bazel(identity: &str) -> String {
    let crate_ident = rust_web_crate_ident(identity);
    format!(
        "# gazelle:resolve rust wasm_bindgen @rules_rust_wasm_bindgen//3rdparty:wasm_bindgen\n# gazelle:resolve rust wasm_bindgen_test @rules_rust_wasm_bindgen//3rdparty:wasm_bindgen_test\nload(\"@rules_dx//rust/rules:defs.bzl\", \"rust_binary\", \"rust_clippy_test\", \"rust_library\", \"rust_test\", \"rust_wasm_bindgen\", \"rust_wasm_bindgen_test\", \"rustfmt_test\")\n\nrust_library(\n    name = \"{identity}\",\n    srcs = [\"src/lib.rs\"],\n    crate_name = \"{crate_ident}\",\n    crate_root = \"src/lib.rs\",\n    edition = \"{edition}\",\n    deps = [\"@rules_rust_wasm_bindgen//3rdparty:wasm_bindgen\"],\n)\n\nrust_binary(\n    name = \"{identity}_bin\",\n    srcs = [\"src/main.rs\"],\n    crate_name = \"{crate_ident}_bin\",\n    crate_root = \"src/main.rs\",\n    edition = \"{edition}\",\n    deps = [\":{identity}\"],\n)\n\nrust_test(\n    name = \"{identity}_test\",\n    crate = \":{identity}\",\n)\n\nrustfmt_test(\n    name = \"{identity}_fmt_test\",\n    size = \"small\",\n    targets = [\n        \":{identity}\",\n        \":{identity}_bin\",\n    ],\n)\n\nrust_clippy_test(\n    name = \"{identity}_clippy_test\",\n    size = \"small\",\n    targets = [\n        \":{identity}\",\n        \":{identity}_bin\",\n    ],\n)\n\nrust_wasm_bindgen(\n    name = \"{identity}_web\",\n    target = \"web\",\n    wasm_file = \":{identity}_bin_upstream\",\n)\n\nrust_wasm_bindgen_test(\n    name = \"{identity}_browser_test\",\n    wasm = \":{identity}_web\",\n)\n",
        edition = RUST_WEB_EDITION,
    )
}

fn rust_web_module_bazel(identity: &str) -> String {
    format!(
        "module(name = \"{identity}\")\n\nbazel_dep(name = \"rules_dx\", version = \"{dx}\")\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\nbazel_dep(name = \"rules_rust_wasm_bindgen\", version = \"{bindgen}\")\nbazel_dep(name = \"platforms\", version = \"{platforms}\")\n",
        dx = super::DX_VERSION,
        bindgen = RULES_RUST_WASM_BINDGEN_VERSION,
        platforms = RUST_WEB_PLATFORMS_VERSION,
    )
}

fn rust_web_readme(identity: &str) -> String {
    format!(
        "# {identity}\n\nShared Rust core with a native binary and a browser build.\n\n```sh\nbazel build //...\nbazel test //...\n```\n\n`bazel test //...` also runs `:{identity}_browser_test` in a browser and\ndriver that Bazel fetches; nothing needs to be installed by hand. The\nbrowser test is qualified on linux x86_64 only, where the host needs GTK 3.\nOther hosts skip it for now.\n"
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

const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "void",
    "volatile",
    "while",
];

/// Returns the application ID `com.example.<crate>` or why the name cannot form one.
fn rust_android_app_id(identity: &str) -> Result<String, AdoptError> {
    let ident = rust_web_crate_ident(identity);
    let refused = |reason: &str| AdoptError::NewInvalidIdentity {
        language: "rust-android".to_owned(),
        name: identity.to_owned(),
        reason: reason.to_owned(),
    };
    if !ident.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err(refused(
            "rust-android names start with a letter so com.example.<name> is a valid application ID",
        ));
    }
    if !ident
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(refused(
            "rust-android names use [a-z0-9_-] only so com.example.<name> is a valid application ID",
        ));
    }
    if JAVA_KEYWORDS.contains(&ident.as_str()) {
        return Err(refused(
            "rust-android names are not Java keywords so com.example.<name> is a valid application ID",
        ));
    }
    Ok(format!("com.example.{ident}"))
}

fn rust_android_cargo_manifest(identity: &str) -> Result<String, AdoptError> {
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
    let mut lib = toml::Table::new();
    lib.insert(
        "crate-type".to_owned(),
        toml::Value::Array(vec![toml::Value::String("staticlib".to_owned())]),
    );
    let mut root = toml::Table::new();
    root.insert("package".to_owned(), toml::Value::Table(package));
    root.insert("lib".to_owned(), toml::Value::Table(lib));
    toml::to_string(&root).map_err(|e| AdoptError::NewInvalidIdentity {
        language: "rust-android".to_owned(),
        name: identity.to_owned(),
        reason: e.to_string(),
    })
}

const RUST_ANDROID_LIB_RS: &str = r#"//! Product core of the Android app; the activity in `activity.c` calls into it.

use std::ffi::c_char;

const GREETING: &[u8] = b"Hello from Rust on Android!\0";

/// Returns the greeting the activity logs at startup.
pub fn greeting() -> &'static str {
    "Hello from Rust on Android!"
}

/// Returns the greeting as a NUL-terminated C string for the activity.
#[no_mangle]
pub extern "C" fn app_greeting() -> *const c_char {
    GREETING.as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::{app_greeting, greeting};
    use std::ffi::CStr;

    #[test]
    fn c_greeting_matches_the_rust_greeting() {
        // SAFETY: app_greeting returns a pointer to a static NUL-terminated string.
        let text = unsafe { CStr::from_ptr(app_greeting()) };
        assert_eq!(text.to_str(), Ok(greeting()));
    }
}
"#;

const RUST_ANDROID_ACTIVITY_C: &str = r#"#include <stddef.h>

#ifdef __ANDROID__
#include <android/log.h>
#include <android/native_activity.h>

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "DxApp", __VA_ARGS__)
#else
typedef struct ANativeActivity ANativeActivity;

#define LOG(...) ((void)0)
#endif

extern const char *app_greeting(void);

__attribute__((visibility("default"))) void
ANativeActivity_onCreate(ANativeActivity *activity, void *saved_state,
                         size_t saved_state_size) {
  (void)activity;
  (void)saved_state;
  (void)saved_state_size;
  LOG("greeting=%s", app_greeting());
}
"#;

const RUST_ANDROID_BUILD: &str = r#"# gazelle:ignore
load("@rules_dx//android/packaging:defs.bzl", "android_adb_install", "android_adb_log", "android_adb_start", "android_apk_check", "android_apk_release_sign", "rust_android_apk")
load("@rules_dx//cc/rules:defs.bzl", "cc_library")
load("@rules_dx//rust/rules:defs.bzl", "rust_clippy_test", "rust_static_library", "rust_test", "rustfmt_test")

rust_static_library(
    name = "@NAME@",
    srcs = ["src/lib.rs"],
    crate_name = "@CRATE@",
    crate_root = "src/lib.rs",
    edition = "@EDITION@",
)

rust_test(
    name = "@NAME@_test",
    crate = ":@NAME@",
)

rustfmt_test(
    name = "@NAME@_fmt_test",
    size = "small",
    targets = [":@NAME@"],
)

rust_clippy_test(
    name = "@NAME@_clippy_test",
    size = "small",
    targets = [":@NAME@"],
)

cc_library(
    name = "@NAME@_activity",
    srcs = ["activity.c"],
    linkopts = [
        "-landroid",
        "-llog",
    ],
    tags = ["manual"],
    deps = [":@NAME@"],
    alwayslink = True,
)

rust_android_apk(
    name = "@NAME@_apk",
    abis = [
        "arm64-v8a",
        "x86_64",
    ],
    api_level = @API@,
    app_id = "@APP_ID@",
    resource_files = ["res/values/strings.xml"],
    version_code = 1,
    version_name = "1.0",
    deps = [":@NAME@_activity"],
)

android_apk_check(
    name = "@NAME@_check_test",
    apk = ":@NAME@_apk_unsigned_file",
    tags = ["manual"],
)

android_apk_release_sign(
    name = "@NAME@_release_sign",
    apk = ":@NAME@_apk_unsigned_file",
)

android_adb_install(
    name = "@NAME@_install",
    apk = ":@NAME@_apk_file",
    serial = "@SERIAL@",
)

android_adb_start(
    name = "@NAME@_start",
    app_id = "@APP_ID@",
    serial = "@SERIAL@",
)

android_adb_log(
    name = "@NAME@_log",
    serial = "@SERIAL@",
)
"#;

fn rust_android_build_bazel(identity: &str, app_id: &str) -> String {
    RUST_ANDROID_BUILD
        .replace("@NAME@", identity)
        .replace("@CRATE@", &rust_web_crate_ident(identity))
        .replace("@EDITION@", RUST_WEB_EDITION)
        .replace("@API@", &RUST_ANDROID_API_LEVEL.to_string())
        .replace("@APP_ID@", app_id)
        .replace("@SERIAL@", RUST_ANDROID_SERIAL)
}

fn rust_android_strings_xml(identity: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n    <string name=\"app_name\">{}</string>\n</resources>\n",
        escape_xml(identity)
    )
}

fn rust_android_module_bazel(identity: &str) -> String {
    format!(
        "{module}bazel_dep(name = \"rules_rust\", version = \"{rules_rust}\")\n\n# The Android Rust targets need the two Android triples in the root toolchain.\nrust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nrust.toolchain(\n    edition = \"{edition}\",\n    extra_target_triples = [\n        \"aarch64-linux-android\",\n        \"x86_64-linux-android\",\n    ],\n    rustfmt_version = \"{rustfmt}\",\n    versions = [\"{rust}\"],\n)\nuse_repo(rust, \"rust_toolchains\")\n\nregister_toolchains(\"@rust_toolchains//:all\")\n\n# The NDK C toolchain reads ANDROID_SDK_ROOT and ANDROID_NDK_REVISION; without them it is empty.\nandroid_ndk_toolchain_repo = use_repo_rule(\"@rules_dx//android/toolchain:repos.bzl\", \"android_ndk_toolchain_repo\")\n\nandroid_ndk_toolchain_repo(\n    name = \"android_ndk_toolchain\",\n    api_level = {api},\n    cxx_runtime = \"static\",\n)\n",
        module = consumer_module_bazel(identity, false),
        rules_rust = crate::scaffold::CONSUMER_RULES_RUST_VERSION,
        edition = crate::scaffold::CONSUMER_RUST_EDITION,
        rustfmt = crate::scaffold::CONSUMER_RUSTFMT_VERSION,
        rust = crate::scaffold::CONSUMER_RUST_VERSION,
        api = RUST_ANDROID_API_LEVEL,
    )
}

fn rust_android_bazelrc() -> String {
    format!(
        "{}build:android --extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator\n",
        consumer_bazelrc()
    )
}

fn rust_android_readme(identity: &str, app_id: &str) -> String {
    format!(
        "# {identity}\n\nRust core packaged as a debug Android APK ({app_id}) for arm64-v8a and x86_64.\n\nThe host build and tests need no Android SDK:\n\n```sh\nbazel build //...\nbazel test //...\n```\n\nThe APK and its launchers are tagged manual. They need an installed Android SDK\nwith platform android-34, build-tools 35.0.0 and an NDK:\n\n```sh\nexport ANDROID_HOME=$HOME/Android/Sdk\nexport ANDROID_SDK_ROOT=$ANDROID_HOME\nexport ANDROID_NDK_REVISION=<installed ndk version>\nbazel build --config=android :{identity}_apk_file\nbazel test --config=android :{identity}_check_test\nbazel run --config=android :{identity}_install\nbazel run --config=android :{identity}_start\nbazel run --config=android :{identity}_log\n```\n\nThe launchers name {serial}. The APK is debug-signed, so no keys are needed.\nThe app ID, ABIs and API level are attributes of `:{identity}_apk` in BUILD.bazel.\n",
        serial = RUST_ANDROID_SERIAL,
    )
}

fn rust_android_files(identity: &str) -> Result<Vec<(String, String)>, AdoptError> {
    let app_id = rust_android_app_id(identity)?;
    Ok(vec![
        (
            "Cargo.toml".to_owned(),
            rust_android_cargo_manifest(identity)?,
        ),
        ("src/lib.rs".to_owned(), RUST_ANDROID_LIB_RS.to_owned()),
        ("activity.c".to_owned(), RUST_ANDROID_ACTIVITY_C.to_owned()),
        (
            "res/values/strings.xml".to_owned(),
            rust_android_strings_xml(identity),
        ),
        (
            "BUILD.bazel".to_owned(),
            rust_android_build_bazel(identity, &app_id),
        ),
        (
            "MODULE.bazel".to_owned(),
            rust_android_module_bazel(identity),
        ),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), rust_android_bazelrc()),
        (
            "README.md".to_owned(),
            rust_android_readme(identity, &app_id),
        ),
    ])
}

fn consumer_module_bazel(identity: &str, platforms: bool) -> String {
    let mut module = format!(
        "module(name = \"{identity}\")\n\nbazel_dep(name = \"rules_dx\", version = \"{}\")\ninclude(\"{}\")\n",
        super::DX_VERSION,
        crate::overrides::OVERRIDES_INCLUDE
    );
    if platforms {
        module.push_str(&format!(
            "bazel_dep(name = \"platforms\", version = \"{RUST_WEB_PLATFORMS_VERSION}\")\n"
        ));
    }
    module
}

fn consumer_bazelrc() -> String {
    "build --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\nbuild --tool_java_language_version=17\n".to_owned()
}

fn consumer_bazelversion() -> String {
    format!("{RUST_WEB_BAZEL_VERSION}\n")
}

fn ruby_files(identity: &str) -> Vec<(String, String)> {
    let build =
        r#"load("@rules_dx//ruby/rules:defs.bzl", "ruby_binary", "ruby_library", "ruby_test")

ruby_library(
    name = "IDENTITY",
    srcs = ["hello.rb"],
)

ruby_binary(
    name = "IDENTITY_bin",
    srcs = ["main.rb"],
    main = "main.rb",
    deps = [":IDENTITY"],
)

ruby_test(
    name = "IDENTITY_test",
    srcs = ["hello_test.rb"],
    deps = [":IDENTITY"],
)
"#
        .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```
"#
    .replace("IDENTITY", identity);
    let checksums: String = crate::scaffold::CONSUMER_PORTABLE_RUBY_CHECKSUMS
        .iter()
        .map(|(name, digest)| format!("        \"{name}\": \"{digest}\",\n"))
        .collect();
    let module = format!(
        "{}bazel_dep(name = \"rules_ruby\", version = \"{rules_ruby}\")\n\nruby = use_extension(\"@rules_ruby//ruby:extensions.bzl\", \"ruby\")\nruby.toolchain(\n    name = \"ruby\",\n    portable_ruby = True,\n    portable_ruby_checksums = {{\n{checksums}    }},\n    version = \"{ruby}\",\n)\nuse_repo(ruby, \"ruby_toolchains\")\n\nregister_toolchains(\"@ruby_toolchains//:all\")\n",
        consumer_module_bazel(identity, false),
        rules_ruby = crate::scaffold::CONSUMER_RULES_RUBY_VERSION,
        ruby = crate::scaffold::CONSUMER_RUBY_VERSION,
        checksums = checksums,
    );
    vec![
        ("MODULE.bazel".to_owned(), module),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "hello.rb".to_owned(),
            "module Hello\n  def self.greet(name)\n    \"hello #{name}\"\n  end\nend\n".to_owned(),
        ),
        (
            "main.rb".to_owned(),
            "require_relative \"hello\"\n\nputs Hello.greet(\"world\")\n".to_owned(),
        ),
        (
            "hello_test.rb".to_owned(),
            "require_relative \"hello\"\n\ngot = Hello.greet(\"world\")\nif got != \"hello world\"\n  warn \"unexpected greeting: #{got}\"\n  exit 1\nend\nputs \"PASS\"\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn zig_files(identity: &str) -> Vec<(String, String)> {
    let build = r#"load("@rules_dx//zig/rules:defs.bzl", "zig_binary", "zig_library", "zig_test")

zig_library(
    name = "IDENTITY",
    import_name = "hello",
    main = "hello.zig",
)

zig_binary(
    name = "IDENTITY_bin",
    main = "main.zig",
    deps = [":IDENTITY"],
)

zig_test(
    name = "IDENTITY_test",
    main = "hello_test.zig",
    deps = [":IDENTITY"],
)
"#
    .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```
"#
    .replace("IDENTITY", identity);
    vec![
        ("MODULE.bazel".to_owned(), consumer_module_bazel(identity, false)),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "hello.zig".to_owned(),
            "const std = @import(\"std\");\n\npub fn greet(buf: []u8, name: []const u8) ![]u8 {\n    return std.fmt.bufPrint(buf, \"hello {s}\", .{name});\n}\n".to_owned(),
        ),
        (
            "main.zig".to_owned(),
            "const std = @import(\"std\");\nconst hello = @import(\"hello\");\n\npub fn main(init: std.process.Init) !void {\n    var buf: [64]u8 = undefined;\n    var out_buf: [64]u8 = undefined;\n    var out: std.Io.File.Writer = .init(.stdout(), init.io, &out_buf);\n    try out.interface.print(\"{s}\\n\", .{try hello.greet(&buf, \"world\")});\n    try out.interface.flush();\n}\n".to_owned(),
        ),
        (
            "hello_test.zig".to_owned(),
            "const std = @import(\"std\");\nconst hello = @import(\"hello\");\n\ntest \"greets by name\" {\n    var buf: [64]u8 = undefined;\n    try std.testing.expectEqualStrings(\"hello world\", try hello.greet(&buf, \"world\"));\n}\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn cc_starter(identity: &str, ext: &str, sources: [String; 4]) -> Vec<(String, String)> {
    let build = r#"load("@rules_dx//cc/rules:defs.bzl", "cc_binary", "cc_library", "cc_test")

cc_library(
    name = "IDENTITY",
    srcs = ["hello.EXT"],
    hdrs = ["hello.h"],
)

cc_binary(
    name = "IDENTITY_bin",
    srcs = ["main.EXT"],
    deps = [":IDENTITY"],
)

cc_test(
    name = "IDENTITY_test",
    srcs = ["hello_test.EXT"],
    deps = [":IDENTITY"],
)
"#
    .replace("IDENTITY", identity)
    .replace("EXT", ext);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```
"#
    .replace("IDENTITY", identity);
    let [hello, header, main, test] = sources;
    vec![
        (
            "MODULE.bazel".to_owned(),
            consumer_module_bazel(identity, false),
        ),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (format!("hello.{ext}"), hello),
        ("hello.h".to_owned(), header),
        (format!("main.{ext}"), main),
        (format!("hello_test.{ext}"), test),
        ("README.md".to_owned(), readme),
    ]
}

/// Kernel archs of the starter: the pinned nvcc's list, `modules/cuda.bzl` `CUDA_ARCHS`.
const CUDA_STARTER_ARCHS: [&str; 2] = ["compute_89:compute_89,sm_89", "sm_120"];

fn cuda_files(identity: &str) -> Vec<(String, String)> {
    let archs: String = CUDA_STARTER_ARCHS
        .iter()
        .map(|arch| format!("        \"{arch}\",\n"))
        .collect();
    let build = r#"load("@rules_dx//cc/rules:defs.bzl", "cc_binary", "cc_test")
load("@rules_dx//cuda/rules:defs.bzl", "CUDA_TARGET", "cuda_library")

cuda_library(
    name = "IDENTITY",
    srcs = ["scale.cu"],
    hdrs = [
        "scale.cuh",
        "scale.h",
    ],
    archs = [
ARCHS    ],
)

cc_binary(
    name = "IDENTITY_bin",
    srcs = ["main.cc"],
    target_compatible_with = CUDA_TARGET,
    deps = [":IDENTITY"],
)

# Checks the host-side logic only; it launches no kernel, so it passes without a GPU.
cc_test(
    name = "IDENTITY_test",
    srcs = ["scale_test.cc"],
    target_compatible_with = CUDA_TARGET,
    deps = [":IDENTITY"],
)
"#
    .replace("IDENTITY", identity)
    .replace("ARCHS", &archs);
    let readme = r#"# IDENTITY

A CUDA starter. The toolkit (13.4) and its host compiler are pinned in `MODULE.bazel`,
so no CUDA install is needed to build. Linux x86_64 only.

```sh
bazel build //...
bazel test //...
bazel run :IDENTITY_bin
```

The binary runs the kernel when a GPU and driver are present and falls back to the CPU
otherwise. The test checks host-side logic only and launches no kernel.

## License

`MODULE.bazel` contains `cuda_license.license(accept = ["cuda-eula"])`. That line accepts
the NVIDIA CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) on your behalf.
Remove it to decline; CUDA targets then fail to build with a message that names the license.
"#
    .replace("IDENTITY", identity);
    let module = format!(
        "{}\n{}",
        consumer_module_bazel(identity, false),
        super::cuda_stanza::render_cuda_module_stanza("rules_dx"),
    );
    vec![
        ("MODULE.bazel".to_owned(), module),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "scale.cuh".to_owned(),
            "#pragma once\n\n__host__ __device__ inline float scale_one(float a, float x) {\n  return a * x;\n}\n".to_owned(),
        ),
        (
            "scale.h".to_owned(),
            "#pragma once\n\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n\nint scale_device_count(void);\nint scale_gpu(int n, float a, const float *x, float *y);\nvoid scale_cpu(int n, float a, const float *x, float *y);\n\n#ifdef __cplusplus\n}\n#endif\n".to_owned(),
        ),
        (
            "scale.cu".to_owned(),
            "#include <cuda_runtime.h>\n\n#include \"scale.cuh\"\n#include \"scale.h\"\n\n__global__ void scale_kernel(int n, float a, const float* x, float* y) {\n  int i = blockIdx.x * blockDim.x + threadIdx.x;\n  if (i < n) {\n    y[i] = scale_one(a, x[i]);\n  }\n}\n\nextern \"C\" int scale_device_count(void) {\n  int count = 0;\n  if (cudaGetDeviceCount(&count) != cudaSuccess) {\n    return 0;\n  }\n  return count;\n}\n\nextern \"C\" int scale_gpu(int n, float a, const float* x, float* y) {\n  size_t bytes = static_cast<size_t>(n) * sizeof(float);\n  float* dx = nullptr;\n  float* dy = nullptr;\n  if (cudaMalloc(&dx, bytes) != cudaSuccess) {\n    return 1;\n  }\n  if (cudaMalloc(&dy, bytes) != cudaSuccess) {\n    cudaFree(dx);\n    return 1;\n  }\n  cudaMemcpy(dx, x, bytes, cudaMemcpyHostToDevice);\n  scale_kernel<<<(n + 255) / 256, 256>>>(n, a, dx, dy);\n  cudaError_t status = cudaDeviceSynchronize();\n  cudaMemcpy(y, dy, bytes, cudaMemcpyDeviceToHost);\n  cudaFree(dx);\n  cudaFree(dy);\n  return status == cudaSuccess ? 0 : 1;\n}\n\nextern \"C\" void scale_cpu(int n, float a, const float* x, float* y) {\n  for (int i = 0; i < n; ++i) {\n    y[i] = scale_one(a, x[i]);\n  }\n}\n".to_owned(),
        ),
        (
            "main.cc".to_owned(),
            "#include <cstdio>\n#include <vector>\n\n#include \"scale.h\"\n\nint main() {\n  const int n = 8;\n  std::vector<float> x(n);\n  std::vector<float> y(n, 0.0f);\n  for (int i = 0; i < n; ++i) {\n    x[i] = static_cast<float>(i);\n  }\n  if (scale_device_count() == 0 || scale_gpu(n, 2.0f, x.data(), y.data()) != 0) {\n    scale_cpu(n, 2.0f, x.data(), y.data());\n  }\n  float sum = 0.0f;\n  for (int i = 0; i < n; ++i) {\n    sum += y[i];\n  }\n  std::printf(\"scaled sum=%g\\n\", sum);\n  return 0;\n}\n".to_owned(),
        ),
        (
            "scale_test.cc".to_owned(),
            "#include <cstdio>\n#include <vector>\n\n#include \"scale.h\"\n\nint main() {\n  const std::vector<float> x = {1.0f, 2.0f, 3.0f};\n  std::vector<float> y(x.size(), 0.0f);\n  scale_cpu(static_cast<int>(x.size()), 2.0f, x.data(), y.data());\n  for (size_t i = 0; i < x.size(); ++i) {\n    if (y[i] != 2.0f * x[i]) {\n      std::fprintf(stderr, \"unexpected value at %zu: %g\\n\", i, y[i]);\n      return 1;\n    }\n  }\n  return 0;\n}\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn c_files(identity: &str) -> Vec<(String, String)> {
    cc_starter(
        identity,
        "c",
        [
            "#include \"hello.h\"\n\n#include <stdio.h>\n\nint hello(const char *name, char *out, size_t size) {\n  return snprintf(out, size, \"hello %s\", name);\n}\n".to_owned(),
            "#pragma once\n\n#include <stddef.h>\n\nint hello(const char *name, char *out, size_t size);\n".to_owned(),
            "#include <stdio.h>\n\n#include \"hello.h\"\n\nint main(void) {\n  char greeting[64];\n  hello(\"world\", greeting, sizeof greeting);\n  puts(greeting);\n  return 0;\n}\n".to_owned(),
            "#include <stdio.h>\n#include <string.h>\n\n#include \"hello.h\"\n\nint main(void) {\n  char greeting[64];\n  hello(\"world\", greeting, sizeof greeting);\n  if (strcmp(greeting, \"hello world\") != 0) {\n    fprintf(stderr, \"unexpected greeting: %s\\n\", greeting);\n    return 1;\n  }\n  return 0;\n}\n".to_owned(),
        ],
    )
}

fn cpp_files(identity: &str) -> Vec<(String, String)> {
    cc_starter(
        identity,
        "cc",
        [
            "#include \"hello.h\"\n\nstd::string Hello(const std::string& name) {\n  return \"hello \" + name;\n}\n".to_owned(),
            "#pragma once\n\n#include <string>\n\nstd::string Hello(const std::string& name);\n".to_owned(),
            "#include <iostream>\n\n#include \"hello.h\"\n\nint main() {\n  std::cout << Hello(\"world\") << \"\\n\";\n  return 0;\n}\n".to_owned(),
            "#include <iostream>\n#include <string>\n\n#include \"hello.h\"\n\nint main() {\n  const std::string greeting = Hello(\"world\");\n  if (greeting != \"hello world\") {\n    std::cerr << \"unexpected greeting: \" << greeting << \"\\n\";\n    return 1;\n  }\n  return 0;\n}\n".to_owned(),
        ],
    )
}

fn elixir_files(identity: &str) -> Vec<(String, String)> {
    let build = r#"load("@rules_dx//elixir/rules:defs.bzl", "elixir_app", "escript_archive", "escript_headers", "ex_unit_test")

elixir_app(
    name = "IDENTITY",
    app_name = "hello",
    app_version = "0.1.0",
)

escript_archive(
    name = "IDENTITY_bin",
    app = ":IDENTITY",
    headers = escript_headers("Hello.CLI"),
)

ex_unit_test(
    name = "IDENTITY_test",
    srcs = [
        "test/hello_test.exs",
        "test/test_helper.exs",
    ],
    deps = [":IDENTITY"],
)
"#
    .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```
"#
    .replace("IDENTITY", identity);
    vec![
        ("MODULE.bazel".to_owned(), consumer_module_bazel(identity, false)),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "lib/hello.ex".to_owned(),
            "defmodule Hello do\n  @moduledoc \"\"\"\n  Greets people.\n  \"\"\"\n\n  @doc \"\"\"\n  Returns a greeting for `name`.\n  \"\"\"\n  def greet(name), do: \"hello \" <> name\nend\n".to_owned(),
        ),
        (
            "lib/hello/cli.ex".to_owned(),
            "defmodule Hello.CLI do\n  @moduledoc \"\"\"\n  Prints a greeting.\n  \"\"\"\n\n  @doc \"\"\"\n  Escript entry point.\n  \"\"\"\n  def main(_args), do: IO.puts(Hello.greet(\"world\"))\nend\n".to_owned(),
        ),
        (
            "test/test_helper.exs".to_owned(),
            "ExUnit.start()\n".to_owned(),
        ),
        (
            "test/hello_test.exs".to_owned(),
            "defmodule HelloTest do\n  use ExUnit.Case, async: true\n\n  test \"greets by name\" do\n    assert Hello.greet(\"world\") == \"hello world\"\n  end\nend\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn erlang_files(identity: &str) -> Vec<(String, String)> {
    let build = r#"load("@rules_dx//erlang/rules:defs.bzl", "DEFAULT_TEST_ERLC_OPTS", "erlang_app", "erlang_bytecode", "erlc_opts", "escript_archive", "eunit", "test_erlang_app")

erlang_app(
    name = "IDENTITY",
    app_description = "Greets people.",
    app_name = "hello",
    app_version = "0.1.0",
)

test_erlang_app(
    name = "IDENTITY_test_app",
    app_description = "Greets people.",
    app_name = "hello",
    app_version = "0.1.0",
)

erlc_opts(
    name = "test_erlc_opts",
    values = DEFAULT_TEST_ERLC_OPTS,
)

erlang_bytecode(
    name = "IDENTITY_test_modules",
    testonly = True,
    srcs = ["test/hello_tests.erl"],
    outs = ["test/hello_tests.beam"],
    erlc_opts = ":test_erlc_opts",
    deps = [":IDENTITY_test_app"],
)

eunit(
    name = "IDENTITY_test",
    compiled_suites = [":IDENTITY_test_modules"],
    target = ":IDENTITY_test_app",
)

escript_archive(
    name = "IDENTITY_bin",
    app = ":IDENTITY",
    headers = [
        "shebang",
        "{emu_args, \"-escript main hello_cli\"}",
    ],
)
"#
    .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```
"#
    .replace("IDENTITY", identity);
    vec![
        ("MODULE.bazel".to_owned(), consumer_module_bazel(identity, false)),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "src/hello.erl".to_owned(),
            "-module(hello).\n\n-export([greet/1]).\n\n-spec greet(string()) -> string().\ngreet(Name) ->\n    \"hello \" ++ Name.\n".to_owned(),
        ),
        (
            "src/hello_cli.erl".to_owned(),
            "-module(hello_cli).\n\n-export([main/1]).\n\n-spec main([string()]) -> ok.\nmain(_Args) ->\n    io:format(\"~s~n\", [hello:greet(\"world\")]).\n".to_owned(),
        ),
        (
            "test/hello_tests.erl".to_owned(),
            "-module(hello_tests).\n\n-include_lib(\"eunit/include/eunit.hrl\").\n\ngreets_by_name_test() ->\n    ?assertEqual(\"hello world\", hello:greet(\"world\")).\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn ocaml_files(identity: &str) -> Vec<(String, String)> {
    let build = r#"load("@rules_dx//ocaml/rules:defs.bzl", "ocaml_binary", "ocaml_library", "ocaml_module", "ocaml_test")

ocaml_module(
    name = "hello",
    struct = "hello.ml",
)

ocaml_library(
    name = "IDENTITY",
    manifest = [":hello"],
)

ocaml_module(
    name = "main",
    struct = "main.ml",
    deps = [":IDENTITY"],
)

ocaml_binary(
    name = "IDENTITY_bin",
    main = ":main",
)

ocaml_module(
    name = "hello_test_main",
    testonly = True,
    struct = "hello_test.ml",
    deps = [":IDENTITY"],
)

ocaml_test(
    name = "IDENTITY_test",
    main = ":hello_test_main",
)
"#
    .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```
"#
    .replace("IDENTITY", identity);
    vec![
        (
            "MODULE.bazel".to_owned(),
            consumer_module_bazel(identity, false),
        ),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "hello.ml".to_owned(),
            "let greet name = \"hello \" ^ name\n".to_owned(),
        ),
        (
            "main.ml".to_owned(),
            "let () = print_endline (Hello.greet \"world\")\n".to_owned(),
        ),
        (
            "hello_test.ml".to_owned(),
            "let () = assert (Hello.greet \"world\" = \"hello world\")\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn haskell_files(identity: &str) -> Vec<(String, String)> {
    let build = r#"load("@rules_dx//haskell/rules:defs.bzl", "HASKELL_COMPATIBLE_WITH", "haskell_binary", "haskell_library", "haskell_test")

haskell_library(
    name = "IDENTITY",
    srcs = ["src/Hello.hs"],
    target_compatible_with = HASKELL_COMPATIBLE_WITH,
    deps = ["@rules_dx//haskell/toolchain:base"],
)

haskell_binary(
    name = "IDENTITY_bin",
    srcs = ["src/Main.hs"],
    target_compatible_with = HASKELL_COMPATIBLE_WITH,
    deps = [
        ":IDENTITY",
        "@rules_dx//haskell/toolchain:base",
    ],
)

haskell_test(
    name = "IDENTITY_test",
    srcs = ["test/HelloTest.hs"],
    target_compatible_with = HASKELL_COMPATIBLE_WITH,
    deps = [
        ":IDENTITY",
        "@rules_dx//haskell/toolchain:base",
    ],
)
"#
    .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```

The targets need Linux x86_64.
"#
    .replace("IDENTITY", identity);
    vec![
        ("MODULE.bazel".to_owned(), consumer_module_bazel(identity, false)),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "src/Hello.hs".to_owned(),
            "module Hello (greet) where\n\ngreet :: String -> String\ngreet name = \"hello \" ++ name\n".to_owned(),
        ),
        (
            "src/Main.hs".to_owned(),
            "module Main (main) where\n\nimport Hello (greet)\n\nmain :: IO ()\nmain = putStrLn (greet \"world\")\n".to_owned(),
        ),
        (
            "test/HelloTest.hs".to_owned(),
            "module Main (main) where\n\nimport Hello (greet)\nimport System.Exit (exitFailure)\n\nmain :: IO ()\nmain = if greet \"world\" == \"hello world\" then putStrLn \"PASS\" else exitFailure\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

fn powershell_files(identity: &str) -> Vec<(String, String)> {
    let build =
        r#"load("@rules_dx//powershell/rules:defs.bzl", "pwsh_binary", "pwsh_library", "pwsh_test")

pwsh_library(
    name = "IDENTITY",
    srcs = [
        "Hello.psd1",
        "Hello.psm1",
    ],
)

pwsh_binary(
    name = "IDENTITY_bin",
    srcs = ["Main.ps1"],
    target_compatible_with = ["@platforms//os:linux"],
    deps = [":IDENTITY"],
)

pwsh_test(
    name = "IDENTITY_test",
    srcs = ["Hello.Tests.ps1"],
    target_compatible_with = ["@platforms//os:linux"],
    deps = [":IDENTITY"],
)
"#
        .replace("IDENTITY", identity);
    let readme = r#"# IDENTITY

```sh
bazel build //...
bazel run :IDENTITY_bin
bazel test //...
```

The targets are Linux only.
"#
    .replace("IDENTITY", identity);
    vec![
        ("MODULE.bazel".to_owned(), consumer_module_bazel(identity, true)),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        ("BUILD.bazel".to_owned(), build),
        (
            "Hello.psd1".to_owned(),
            "@{\n    RootModule = 'Hello.psm1'\n    ModuleVersion = '0.1.0'\n    GUID = '5b0f3a52-7c1e-4d0e-9a57-3f1c2b8e6d41'\n    PowerShellVersion = '7.0'\n}\n".to_owned(),
        ),
        (
            "Hello.psm1".to_owned(),
            "function Get-Greeting {\n    [OutputType([string])]\n    param([string]$Name = \"world\")\n    return \"hello $Name\"\n}\n".to_owned(),
        ),
        (
            "Main.ps1".to_owned(),
            "#!/usr/bin/env pwsh\n$ErrorActionPreference = \"Stop\"\nImport-Module (Join-Path $PSScriptRoot \"Hello.psm1\") -Force\nWrite-Output (Get-Greeting)\n".to_owned(),
        ),
        (
            "Hello.Tests.ps1".to_owned(),
            "#!/usr/bin/env pwsh\n$ErrorActionPreference = \"Stop\"\nImport-Module (Join-Path $PSScriptRoot \"Hello.psm1\") -Force\n$got = Get-Greeting\nif ($got -ne \"hello world\") {\n    Write-Error \"unexpected greeting: $got\"\n    exit 1\n}\nexit 0\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ]
}

struct JsStarter {
    rule: &'static str,
    component: &'static str,
    package: &'static str,
    version: &'static str,
    lock: &'static str,
    component_source: &'static str,
    test_source: Option<&'static str>,
}

fn js_starter_files(identity: &str, starter: &JsStarter) -> Vec<(String, String)> {
    let module = format!(
        "module(name = \"{identity}\")\n\nbazel_dep(name = \"rules_dx\", version = \"{dx}\")\ninclude(\"//third_party/rules_dx:rules_dx.MODULE.bazel\")\nbazel_dep(name = \"aspect_rules_js\", version = \"{js}\")\n\nnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(\n    name = \"demo_npm\",\n    npmrc = \"//:.npmrc\",\n    pnpm_lock = \"//:pnpm-lock.yaml\",\n    verify_node_modules_ignored = \"//:.bazelignore\",\n)\nuse_repo(npm, \"demo_npm\")\n",
        dx = super::DX_VERSION,
        js = ASPECT_RULES_JS_VERSION,
    );
    let (js_loads, rule_loads, check) = match starter.test_source {
        Some(_) => (
            "\"javascript_js_test\", \"javascript_library\"",
            format!("\"{}_library\"", starter.rule),
            format!(
                r#"javascript_js_test(
    name = "IDENTITY_test",
    size = "small",
    srcs = ["hello.test.js"],
    data = [
        ":IDENTITY",
        ":node_modules/{package}",
    ],
    entry_point = "hello.test.js",
    package_json = ":package_json",
)
"#,
                package = starter.package,
            ),
        ),
        None => (
            "\"javascript_library\"",
            format!("\"{0}_check\", \"{0}_library\"", starter.rule),
            format!(
                r#"{rule}_check(
    name = "IDENTITY_check",
    srcs = ["{component}"],
    node_modules = ":node_modules",
    package_json = ":package_json",
)
"#,
                rule = starter.rule,
                component = starter.component,
            ),
        ),
    };
    let build = format!(
        r#"load("@aspect_rules_js//js:defs.bzl", "js_library")
load("@demo_npm//:defs.bzl", "npm_link_all_packages")
load("@rules_dx//javascript/rules:defs.bzl", {js_loads})
load("@rules_dx//{rule}/rules:defs.bzl", {rule_loads})

npm_link_all_packages(name = "node_modules")

js_library(
    name = "package_json",
    srcs = ["package.json"],
)

javascript_library(
    name = "helper",
    srcs = ["helper.js"],
)

{rule}_library(
    name = "IDENTITY",
    srcs = ["{component}"],
    deps = [":helper"],
)

{check}"#,
        rule = starter.rule,
        component = starter.component,
    )
    .replace("IDENTITY", identity);
    let package_json = format!(
        "{{\n  \"name\": \"{}\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"devDependencies\": {{\n    \"{}\": \"{}\"\n  }}\n}}\n",
        escape_json_string(identity),
        starter.package,
        starter.version
    );
    let readme = format!("# {identity}\n\n```sh\nbazel build //...\nbazel test //...\n```\n");
    let mut files = vec![
        ("MODULE.bazel".to_owned(), module),
        (".bazelversion".to_owned(), consumer_bazelversion()),
        (".bazelrc".to_owned(), consumer_bazelrc()),
        (".bazelignore".to_owned(), ".dx\nnode_modules\n".to_owned()),
        (".npmrc".to_owned(), "hoist=false\n".to_owned()),
        ("BUILD.bazel".to_owned(), build),
        ("package.json".to_owned(), package_json),
        ("pnpm-lock.yaml".to_owned(), starter.lock.to_owned()),
        (
            "pnpm-workspace.yaml".to_owned(),
            "packages:\n  - .\nallowBuilds: {}\n".to_owned(),
        ),
        (
            starter.component.to_owned(),
            starter.component_source.to_owned(),
        ),
        (
            "helper.js".to_owned(),
            "export function helper(name) {\n  return `hello ${name}`;\n}\n".to_owned(),
        ),
        ("README.md".to_owned(), readme),
    ];
    if let Some(test_source) = starter.test_source {
        files.push(("hello.test.js".to_owned(), test_source.to_owned()));
    }
    files
}

fn astro_files(identity: &str) -> Vec<(String, String)> {
    js_starter_files(
        identity,
        &JsStarter {
            rule: "astro",
            component: "Hello.astro",
            package: "@astrojs/compiler",
            version: ASTRO_COMPILER_VERSION,
            lock: super::new_locks::ASTRO_LOCK,
            component_source: "---\nimport { helper } from \"./helper.js\";\n\nconst message = helper(\"world\");\n---\n\n<div class=\"hello\">{message}</div>\n",
            test_source: Some(
                "import assert from \"node:assert/strict\";\nimport fs from \"node:fs\";\nimport path from \"node:path\";\nimport { fileURLToPath } from \"node:url\";\nimport { parse } from \"@astrojs/compiler/sync\";\nimport { is } from \"@astrojs/compiler/utils\";\n\nconst here = path.dirname(fileURLToPath(import.meta.url));\nconst result = parse(fs.readFileSync(path.join(here, \"Hello.astro\"), \"utf8\"));\n\nassert.deepEqual(result.diagnostics, []);\nassert.ok(result.ast.children.some((node) => is.frontmatter(node)));\nassert.ok(\n  result.ast.children.some((node) => is.element(node) && node.name === \"div\"),\n);\nconsole.log(\"PASS\");\n",
            ),
        },
    )
}

fn svelte_files(identity: &str) -> Vec<(String, String)> {
    js_starter_files(
        identity,
        &JsStarter {
            rule: "svelte",
            component: "Hello.svelte",
            package: "svelte",
            version: SVELTE_VERSION,
            lock: super::new_locks::SVELTE_LOCK,
            component_source: "<script>\n  import { helper } from \"./helper.js\";\n\n  let name = \"world\";\n  let message = $derived(helper(name));\n</script>\n\n<div class=\"hello\">{message}</div>\n",
            test_source: Some(
                "import assert from \"node:assert/strict\";\nimport fs from \"node:fs\";\nimport path from \"node:path\";\nimport { fileURLToPath } from \"node:url\";\nimport { parse } from \"svelte/compiler\";\n\nconst here = path.dirname(fileURLToPath(import.meta.url));\nconst source = fs.readFileSync(path.join(here, \"Hello.svelte\"), \"utf8\");\nconst ast = parse(source, { modern: true });\n\nassert.ok(ast.fragment.nodes.length > 0);\nassert.notEqual(ast.instance, null);\nassert.ok(\n  source.slice(ast.instance.start, ast.instance.end).includes(\"./helper.js\"),\n);\nconsole.log(\"PASS\");\n",
            ),
        },
    )
}

fn vue_files(identity: &str) -> Vec<(String, String)> {
    js_starter_files(
        identity,
        &JsStarter {
            rule: "vue",
            component: "Hello.vue",
            package: "@vue/compiler-sfc",
            version: VUE_COMPILER_VERSION,
            lock: super::new_locks::VUE_LOCK,
            component_source: "<template>\n  <div class=\"hello\">{{ message }}</div>\n</template>\n\n<script setup>\nimport { helper } from \"./helper.js\";\n\nconst message = helper(\"world\");\n</script>\n",
            test_source: Some(
                "import assert from \"node:assert/strict\";\nimport fs from \"node:fs\";\nimport path from \"node:path\";\nimport { fileURLToPath } from \"node:url\";\nimport { parse } from \"@vue/compiler-sfc\";\n\nconst here = path.dirname(fileURLToPath(import.meta.url));\nconst source = fs.readFileSync(path.join(here, \"Hello.vue\"), \"utf8\");\nconst { descriptor, errors } = parse(source);\n\nassert.deepEqual(errors, []);\nassert.notEqual(descriptor.template, null);\nassert.ok(descriptor.scriptSetup.content.includes(\"./helper.js\"));\nconsole.log(\"PASS\");\n",
            ),
        },
    )
}

fn mdx_files(identity: &str) -> Vec<(String, String)> {
    js_starter_files(
        identity,
        &JsStarter {
            rule: "mdx",
            component: "Hello.mdx",
            package: "@mdx-js/mdx",
            version: MDX_VERSION,
            lock: super::new_locks::MDX_LOCK,
            component_source: "import { helper } from \"./helper.js\";\n\n# Hello\n\n<div className=\"hello\">{helper(\"world\")}</div>\n",
            test_source: None,
        },
    )
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
        "rust-android" => rust_android_files(identity)?,
        "python" => vec![
            ("pyproject.toml".to_owned(), python_manifest(identity)?),
            (
                "hello.py".to_owned(),
                "\"\"\"Greeting helper with no dependencies.\"\"\"\n\n\ndef greet(name):\n    \"\"\"Return a greeting for name.\"\"\"\n    return f\"Hello, {name}!\"\n".to_owned(),
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
        "ruby" => ruby_files(identity),
        "powershell" => powershell_files(identity),
        "astro" => astro_files(identity),
        "svelte" => svelte_files(identity),
        "vue" => vue_files(identity),
        "mdx" => mdx_files(identity),
        "zig" => zig_files(identity),
        "elixir" => elixir_files(identity),
        "erlang" => erlang_files(identity),
        "ocaml" => ocaml_files(identity),
        "haskell" => haskell_files(identity),
        "cuda" => cuda_files(identity),
        "c" => c_files(identity),
        "cpp" => cpp_files(identity),
        _ => {
            return Err(AdoptError::NewUnknownLanguage {
                language: canonical.to_owned(),
            })
        }
    };
    Ok(files)
}

pub fn apply_new(
    root: &Path,
    language: &str,
    name: &str,
    applied: &mut crate::scaffold::Applied,
) -> Result<(), AdoptError> {
    crate::scaffold::apply_scaffold(root, &plan_new_files(language, name)?, applied)
}

#[cfg(test)]
mod tests {
    use super::super::DX_VERSION;
    use super::*;

    use super::super::version::repo_text as workspace_text;

    #[test]
    fn new_keeps_every_init_part() {
        let files = plan_new_files("rust", "demo").expect("plans");
        for part in crate::InitPart::ALL {
            for path in part.scaffold_paths() {
                assert!(
                    files.iter().any(|f| f.path == format!("demo/{path}")),
                    "{path}"
                );
            }
        }
    }

    #[test]
    fn js_starter_pins_match_repository_pins() {
        let module = workspace_text("MODULE.bazel");
        assert!(module.contains(&format!(
            "bazel_dep(name = \"aspect_rules_js\", version = \"{ASPECT_RULES_JS_VERSION}\")"
        )));
        let package = workspace_text("package.json");
        assert!(package.contains(&format!(
            "\"@astrojs/compiler\": \"{ASTRO_COMPILER_VERSION}\""
        )));
        assert!(package.contains(&format!("\"svelte\": \"{SVELTE_VERSION}\"")));
    }

    #[test]
    fn js_starters_pin_one_framework_package_and_no_extras() {
        for (language, package, version) in [
            ("astro", "@astrojs/compiler", ASTRO_COMPILER_VERSION),
            ("svelte", "svelte", SVELTE_VERSION),
            ("vue", "@vue/compiler-sfc", VUE_COMPILER_VERSION),
            ("mdx", "@mdx-js/mdx", MDX_VERSION),
        ] {
            let files = plan_new_files(language, "demo-app").expect("plans");
            let read = |name: &str| {
                files
                    .iter()
                    .find(|f| f.path == format!("demo-app/{name}"))
                    .unwrap_or_else(|| panic!("{language}: {name}"))
                    .content
                    .clone()
            };
            let manifest = read("package.json");
            assert!(
                manifest.contains(&format!("\"{package}\": \"{version}\"")),
                "{manifest}"
            );
            assert_eq!(manifest.matches("\": \"").count(), 3, "{manifest}");
            assert!(read("pnpm-lock.yaml").contains(&format!("specifier: {version}")));
            let build = read("BUILD.bazel");
            assert!(build.contains(&format!("{language}_library(")), "{build}");
            if language == "mdx" {
                assert!(build.contains("mdx_check("), "{build}");
                assert!(!build.contains("javascript_js_test("), "{build}");
                assert!(!files.iter().any(|f| f.path.ends_with("hello.test.js")));
            } else {
                assert!(build.contains("javascript_js_test("), "{build}");
            }
            assert!(
                build.contains("package_json = \":package_json\""),
                "{build}"
            );
            assert!(!build.contains("jest"), "{build}");
            let module = read("MODULE.bazel");
            assert!(module.contains("module(name = \"demo-app\")"), "{module}");
        }
    }

    #[test]
    fn ruby_scaffold_hashes_the_portable_ruby_downloads() {
        let files = ruby_files("demo");
        let module = &files
            .iter()
            .find(|(path, _)| path == "MODULE.bazel")
            .expect("MODULE.bazel")
            .1;
        assert!(module.contains("portable_ruby_checksums"));
        for (name, digest) in crate::scaffold::CONSUMER_PORTABLE_RUBY_CHECKSUMS {
            assert!(module.contains(&format!("\"{name}\": \"{digest}\"")));
        }
    }

    #[test]
    fn new_aliases_normalize_to_canonical_templates() {
        assert_eq!(normalize_new_language("rust"), Some("rust"));
        assert_eq!(normalize_new_language("c"), Some("c"));
        assert_eq!(normalize_new_language("cpp"), Some("cpp"));
        for spelling in ["cc", "c#", "c-sharp", "f#", "f-sharp"] {
            assert_eq!(normalize_new_language(spelling), None);
            assert!(removed_new_language(spelling).is_some());
        }
        assert_eq!(normalize_new_language("ruby"), Some("ruby"));
        assert_eq!(normalize_new_language("powershell"), Some("powershell"));
        assert!(new_is_known_language("go"));
        assert!(new_is_known_language("rust-web"));
        assert!(!new_is_known_language("swift"));
        assert_eq!(normalize_new_language("zig"), Some("zig"));
        assert_eq!(normalize_new_language("elixir"), Some("elixir"));
        assert_eq!(normalize_new_language("erlang"), Some("erlang"));
        assert_eq!(normalize_new_language("ocaml"), Some("ocaml"));
        assert_eq!(normalize_new_language("haskell"), Some("haskell"));
        assert_eq!(normalize_new_language("cuda"), Some("cuda"));
    }

    #[test]
    fn cuda_starter_carries_the_pinned_stanza_and_known_archs() {
        let files = plan_new_files("cuda", "demo-app").expect("plans");
        let text = |name: &str| {
            files
                .iter()
                .find(|file| file.path == format!("demo-app/{name}"))
                .unwrap_or_else(|| panic!("{name} planned"))
                .content
                .clone()
        };
        let module = text("MODULE.bazel");
        assert!(module.starts_with("module(name = \"demo-app\")"));
        assert!(module.ends_with(&crate::cuda_stanza::render_cuda_module_stanza("rules_dx")));
        assert!(text("README.md").contains("https://docs.nvidia.com/cuda/eula/index.html"));
        let cuda = workspace_text("modules/cuda.bzl");
        let build = text("BUILD.bazel");
        for arch in CUDA_STARTER_ARCHS {
            for part in arch.split([':', ',']) {
                assert!(
                    cuda.contains(&format!("\"{part}\"")),
                    "{part} is not in CUDA_ARCHS"
                );
            }
            assert!(build.contains(arch));
        }
    }

    #[test]
    fn new_plan_paths_are_unique_and_js_starters_ignore_dx_and_node_modules() {
        for language in SUPPORTED_NEW_LANGUAGES {
            let files = plan_new_files(language, "demo").expect("plans");
            let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
            let total = paths.len();
            paths.sort_unstable();
            paths.dedup();
            assert_eq!(paths.len(), total, "{language} plans a path twice");
        }
        let files = plan_new_files("astro", "demo").expect("plans");
        let ignores: Vec<&ScaffoldFile> = files
            .iter()
            .filter(|file| file.path == "demo/.bazelignore")
            .collect();
        assert_eq!(ignores.len(), 1);
        assert_eq!(ignores[0].content, ".dx\nnode_modules\n");
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
        for (lang, ext, other) in [("c", "c", "cc"), ("cpp", "cc", "c")] {
            let files = plan_new_files(lang, "demo").expect(lang);
            for path in [
                "demo/MODULE.bazel".to_owned(),
                "demo/.bazelversion".to_owned(),
                "demo/.bazelrc".to_owned(),
                "demo/BUILD.bazel".to_owned(),
                "demo/README.md".to_owned(),
                "demo/hello.h".to_owned(),
                format!("demo/hello.{ext}"),
                format!("demo/main.{ext}"),
                format!("demo/hello_test.{ext}"),
            ] {
                assert!(files.iter().any(|f| f.path == path), "{lang} lacks {path}");
            }
            assert!(
                !files.iter().any(|f| f.path.ends_with(&format!(".{other}"))),
                "{lang} writes no .{other} file"
            );
            let build = files
                .iter()
                .find(|f| f.path == "demo/BUILD.bazel")
                .expect("build");
            assert!(build
                .content
                .starts_with("load(\"@rules_dx//cc/rules:defs.bzl\", \"cc_binary\", \"cc_library\", \"cc_test\")\n"));
            for name in [
                format!("srcs = [\"hello.{ext}\"]"),
                format!("srcs = [\"main.{ext}\"]"),
                format!("srcs = [\"hello_test.{ext}\"]"),
                "name = \"demo_bin\"".to_owned(),
                "name = \"demo_test\"".to_owned(),
            ] {
                assert!(
                    build.content.contains(&name),
                    "{lang}: {name}\n{}",
                    build.content
                );
            }
            assert!(
                !build.content.contains(&format!("hello.{other}\"")),
                "{}",
                build.content
            );
            let module = files
                .iter()
                .find(|f| f.path == "demo/MODULE.bazel")
                .expect("module");
            assert!(module.content.starts_with("module(name = \"demo\")\n"));
            assert!(
                module.content.contains(&format!(
                    "bazel_dep(name = \"rules_dx\", version = \"{}\")\ninclude(\"{}\")\n",
                    crate::DX_VERSION,
                    crate::overrides::OVERRIDES_INCLUDE
                )),
                "{lang}: {}",
                module.content
            );
            let error = plan_new_files(lang, "1demo").unwrap_err();
            assert!(
                matches!(error, AdoptError::NewInvalidIdentity { .. }),
                "{lang}: {error}"
            );
            let spaced = plan_new_files(lang, "My App").expect("folds");
            let module = spaced
                .iter()
                .find(|f| f.path == "My App/MODULE.bazel")
                .expect("module");
            assert!(module.content.starts_with("module(name = \"my-app\")\n"));
        }
        let c = plan_new_files("c", "demo").expect("c");
        let text = |path: &str| {
            c.iter()
                .find(|f| f.path == path)
                .map(|f| f.content.clone())
                .expect(path)
        };
        assert!(text("demo/hello_test.c").contains("strcmp"));
        assert!(!text("demo/hello.h").contains("<string>"));
        let cpp = plan_new_files("cpp", "demo").expect("cpp");
        let test = cpp
            .iter()
            .find(|f| f.path == "demo/hello_test.cc")
            .expect("test");
        assert!(test.content.contains("std::string"));
        for (spelling, canonical) in [("cc", "cpp"), ("c#", "csharp"), ("f-sharp", "fsharp")] {
            let message = plan_new_files(spelling, "demo").unwrap_err().to_string();
            assert!(message.contains(canonical), "{message}");
        }
        let version = rust
            .iter()
            .find(|f| f.path == "demo/.dx/version")
            .expect("version");
        assert_eq!(version.content, format!("{DX_VERSION}\n"));
        assert!(plan_new_files("swift", "demo").is_err());
        assert_eq!(
            plan_new_files("swift", "demo").unwrap_err().to_string(),
            format!(
                "unknown language for dx new: swift (want one of {})",
                new_language_name_list()
            )
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
            "[dependencies]\nwasm-bindgen = \"=0.2.121\"\n\n[dev-dependencies]\nwasm-bindgen-test = \"=0.3.71\"\n\n[package]\nedition = \"2021\"\nname = \"demo\"\nversion = \"0.1.0\"\n"
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
            "# gazelle:resolve rust wasm_bindgen_test @rules_rust_wasm_bindgen//3rdparty:wasm_bindgen_test",
        ] {
            assert!(build.content.contains(wanted), "{wanted}");
        }
        assert_eq!(
            main.content.matches("#[wasm_bindgen_test]").count(),
            2,
            "{main:?}"
        );
        assert!(
            !paths.iter().any(|path| path.ends_with(".bazelrc")),
            "{paths:?}"
        );
        for file in files.iter().filter(|file| {
            [
                "demo/Cargo.toml",
                "demo/src/lib.rs",
                "demo/src/main.rs",
                "demo/BUILD.bazel",
                "demo/MODULE.bazel",
                "demo/README.md",
            ]
            .contains(&file.path.as_str())
        }) {
            for banned in ["geckodriver", "GECKODRIVER", "firefox", "Firefox", "manual"] {
                assert!(
                    !file.content.contains(banned),
                    "{} carries {banned}: {}",
                    file.path,
                    file.content
                );
            }
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

    fn planned(files: &[ScaffoldFile], path: &str) -> String {
        files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("no {path}"))
            .content
            .clone()
    }

    #[test]
    fn rust_android_template_writes_host_and_apk_files_with_explicit_policy() {
        let files = plan_new_files("rust-android", "demo-app").expect("plans");
        for wanted in [
            "demo-app/Cargo.toml",
            "demo-app/src/lib.rs",
            "demo-app/activity.c",
            "demo-app/res/values/strings.xml",
            "demo-app/BUILD.bazel",
            "demo-app/MODULE.bazel",
            "demo-app/.bazelversion",
            "demo-app/.bazelrc",
            "demo-app/README.md",
            "demo-app/.dx/version",
        ] {
            assert!(files.iter().any(|f| f.path == wanted), "no {wanted}");
        }
        let build = planned(&files, "demo-app/BUILD.bazel");
        for wanted in [
            "name = \"demo-app_apk\"",
            "crate_name = \"demo_app\"",
            "app_id = \"com.example.demo_app\"",
            "api_level = 31,",
            "\"arm64-v8a\",\n        \"x86_64\",",
            "version_code = 1,",
            "version_name = \"1.0\",",
            "name = \"demo-app_install\"",
            "name = \"demo-app_start\"",
            "name = \"demo-app_log\"",
            "name = \"demo-app_check_test\"",
            "serial = \"emulator-5554\"",
        ] {
            assert!(build.contains(wanted), "{wanted}\n{build}");
        }
        let module = planned(&files, "demo-app/MODULE.bazel");
        assert!(
            module.starts_with(&consumer_module_bazel("demo-app", false)),
            "{module}"
        );
        for wanted in [
            "\"aarch64-linux-android\"",
            "\"x86_64-linux-android\"",
            "use_repo_rule(\"@rules_dx//android/toolchain:repos.bzl\", \"android_ndk_toolchain_repo\")",
            "api_level = 31,",
            "cxx_runtime = \"static\",",
        ] {
            assert!(module.contains(wanted), "{wanted}\n{module}");
        }
        for file in &files {
            for refused in [
                "accept_licenses",
                "android_sdk.configure",
                "release_sign",
                "keystore",
            ] {
                assert!(
                    !file.content.contains(refused),
                    "{} carries {refused}",
                    file.path
                );
            }
        }
        let bazelrc = planned(&files, "demo-app/.bazelrc");
        assert!(bazelrc.starts_with(&consumer_bazelrc()), "{bazelrc}");
        assert!(
            bazelrc.contains("build:android --extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator\n"),
            "{bazelrc}"
        );
        assert_eq!(
            planned(&files, "demo-app/.bazelversion"),
            consumer_bazelversion()
        );
        let strings = planned(&files, "demo-app/res/values/strings.xml");
        assert!(strings.contains(">demo-app</string>"), "{strings}");
        let readme = planned(&files, "demo-app/README.md");
        assert!(
            readme.contains("bazel run --config=android :demo-app_install"),
            "{readme}"
        );
    }

    #[test]
    fn rust_android_names_fold_to_valid_app_ids_or_are_refused() {
        for (name, app_id) in [
            ("demo", "com.example.demo"),
            ("My App", "com.example.my_app"),
            ("teams/Demo-App2", "com.example.demo_app2"),
        ] {
            let files = plan_new_files("rust-android", name).expect(name);
            let build = files
                .iter()
                .find(|f| f.path == format!("{name}/BUILD.bazel"))
                .expect("build");
            assert!(
                build.content.contains(&format!("app_id = \"{app_id}\"")),
                "{name}: {}",
                build.content
            );
        }
        for name in ["1demo", "+", "class", "do"] {
            let error = plan_new_files("rust-android", name).unwrap_err();
            assert!(
                matches!(error, AdoptError::NewInvalidIdentity { .. }),
                "{name}: {error}"
            );
        }
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
            crate::scaffold::entries(|a| apply_new(scratch.path(), "rust", "demo", a)),
            Err(AdoptError::NotADirectory { .. })
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
    fn python_scaffold_greet_interpolates_name() {
        let scratch = dx_test_scratch::scratch("dx-new-python-");
        let root = scratch.path().to_path_buf();
        crate::scaffold::entries(|a| apply_new(&root, "python", "demo", a)).expect("new");
        let dir = root.join("demo");
        let python = std::env::var("DX_PYTHON").expect("DX_PYTHON");
        let python = std::fs::canonicalize(python).expect("python path");
        let script = "import hello\nfor n in ('Ada', 'Grace', ''):\n    print(hello.greet(n))\n";
        let out = std::process::Command::new(python)
            .current_dir(&dir)
            .args(["-S", "-c", script])
            .env("PYTHONPATH", &dir)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .expect("python runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
            "Hello, Ada!\nHello, Grace!\nHello, !\n"
        );
    }

    #[test]
    fn apply_new_writes_absent_only_and_refuses_existing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = crate::scaffold::entries(|a| apply_new(&root, "go", "demo", a)).expect("new");
        assert!(first.iter().any(|p| p == "demo/go.mod"));
        assert!(root.join("demo/go.mod").exists());
        assert!(root.join("demo/.dx/version").exists());
        let second =
            crate::scaffold::entries(|a| apply_new(&root, "go", "demo", a)).expect("new again");
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
        assert!(ci
            .content
            .contains("uses: devhusdk/rules_dx/.github/workflows/"));
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
                crate::scaffold::entries(|a| apply_new(&root, "rust", hostile, a)).is_err(),
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
        let scratch = dx_test_scratch::scratch("dx-new-dest-");
        let root = scratch.path();
        assert!(crate::scaffold::scaffold_dest_within_root(root, "demo/a").is_ok());
        for hostile in ["../evil", "/absolute", "", "a/../../b"] {
            assert!(
                crate::scaffold::scaffold_dest_within_root(root, hostile).is_err(),
                "{hostile:?} escapes"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn new_refuses_project_directory_symlinked_outside_root() {
        let scratch = dx_test_scratch::scratch("dx-adopt-new-symlink-");
        let root = scratch.path().join("root");
        let outside = scratch.path().join("outside");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::create_dir_all(&outside).expect("outside");
        std::os::unix::fs::symlink(&outside, root.join("demo")).expect("symlink");
        assert!(crate::scaffold::entries(|a| apply_new(&root, "rust", "demo", a)).is_err());
        assert_eq!(std::fs::read_dir(&outside).expect("read").count(), 0);
        scratch.close().expect("cleanup");
    }
}
