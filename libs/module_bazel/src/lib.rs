//! One reader for literal top-level `MODULE.bazel` calls, shared by every MODULE.bazel tool.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod identity;

use std::collections::BTreeSet;
use std::ops::Range;

use starlark_syntax::codemap::Span;
use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstExpr, AstLiteral, AstStmt, ExprP, StmtP,
};
use starlark_syntax::syntax::{AstModule, Dialect};

/// A string literal with its decoded value and the byte range of its token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Str {
    pub value: String,
    pub span: Range<usize>,
}

/// One call argument, kept only as far as it is literal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Str(Str),
    List(Vec<Value>),
    Dict(Vec<(Value, Value)>),
    Ident(String),
    Computed { line: usize },
}

/// One top-level call such as `bazel_dep(...)` or `maven.install(...)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    pub name: String,
    pub binding: Option<String>,
    pub line: usize,
    /// Byte range of the call expression.
    pub span: Range<usize>,
    pub positional: Vec<Value>,
    pub named: Vec<(String, Value)>,
    pub unpacked: bool,
}

impl Call {
    /// The value passed for keyword `key`.
    pub fn arg(&self, key: &str) -> Option<&Value> {
        self.named
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// The literal string passed for keyword `key`.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.arg(key) {
            Some(Value::Str(literal)) => Some(&literal.value),
            _ => None,
        }
    }

    /// The literal strings of the list passed for keyword `key`.
    pub fn str_list(&self, key: &str) -> Vec<&str> {
        match self.arg(key) {
            Some(Value::List(items)) => items
                .iter()
                .filter_map(|item| match item {
                    Value::Str(literal) => Some(literal.value.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Why a MODULE.bazel read or edit cannot proceed safely.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ModuleError {
    #[error("MODULE.bazel does not parse at line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("Starlark file does not parse at line {line}: {message}")]
    StarlarkSyntax { line: usize, message: String },
    #[error("no bazel_dep {name:?} in MODULE.bazel")]
    NotFound { name: String },
    #[error("{count} bazel_dep calls declare {name:?} in MODULE.bazel")]
    Ambiguous { name: String, count: usize },
    #[error("bazel_dep at line {line} has no literal name")]
    ComputedName { line: usize },
    #[error("bazel_dep {name:?} at line {line} has no version")]
    MissingVersion { name: String, line: usize },
    #[error("bazel_dep {name:?} at line {line} has a computed version")]
    ComputedVersion { name: String, line: usize },
    #[error("maven artifacts at line {line} are not a list of string literals")]
    ComputedArtifacts { line: usize },
    #[error("crate.from_cargo at line {line} needs literal name, cargo_lockfile and manifests")]
    ComputedCargoHub { line: usize },
    #[error("maven.install at line {line} needs a literal name and lock_file")]
    ComputedMavenHub { line: usize },
    #[error("{tag} at line {line} needs literal arguments")]
    ComputedHub { tag: &'static str, line: usize },
    #[error("{value:?} cannot be written into a Starlark string literal")]
    UnsafeString { value: String },
    #[error("{name:?} is already declared in MODULE.bazel")]
    AlreadyDeclared { name: String },
    #[error("cannot insert into MODULE.bazel: {reason}")]
    NoInsertPoint { reason: String },
    #[error("no single_version_override for {name:?} in MODULE.bazel")]
    OverrideNotFound { name: String },
    #[error("{count} single_version_override calls name {name:?} in MODULE.bazel")]
    OverrideAmbiguous { name: String, count: usize },
    #[error("single_version_override at line {line} has no literal module_name")]
    OverrideComputedName { line: usize },
    #[error("single_version_override {name:?} at line {line} has no version")]
    OverrideMissingVersion { name: String, line: usize },
    #[error("single_version_override {name:?} at line {line} has a computed version")]
    OverrideComputedVersion { name: String, line: usize },
    #[error("{value:?} cannot be written into a one-line comment")]
    UnsafeComment { value: String },
}

/// The marker that starts the comment line naming why a `bazel_dep` has its floor version.
pub const FLOOR_MARKER: &str = "# floor: ";

/// A literal tag argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arg {
    Str(String),
    List(Vec<String>),
}

/// A tag to add to an extension, such as `from_cargo` with its keyword arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewTag {
    pub name: String,
    pub args: Vec<(String, Arg)>,
}

impl NewTag {
    fn check(&self) -> Result<(), ModuleError> {
        check_safe(&self.name)?;
        for (key, arg) in &self.args {
            check_safe(key)?;
            match arg {
                Arg::Str(value) => check_safe(value)?,
                Arg::List(values) => values.iter().try_for_each(|v| check_safe(v))?,
            }
        }
        Ok(())
    }

    fn render(&self, binding: &str) -> String {
        let mut out = format!("{binding}.{}(\n", self.name);
        for (key, arg) in &self.args {
            match arg {
                Arg::Str(value) => out.push_str(&format!("    {key} = \"{value}\",\n")),
                Arg::List(values) if values.len() == 1 => {
                    out.push_str(&format!("    {key} = [\"{}\"],\n", values[0]));
                }
                Arg::List(values) => {
                    out.push_str(&format!("    {key} = [\n"));
                    for value in values {
                        out.push_str(&format!("        \"{value}\",\n"));
                    }
                    out.push_str("    ],\n");
                }
            }
        }
        out.push(')');
        out
    }
}

/// One rules_rust `crate.from_cargo` tag: a crate_universe hub and the manifests it reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoHub<'a> {
    pub name: &'a str,
    pub cargo_lockfile: Option<&'a str>,
    /// The literal `lockfile` label (the cargo-bazel lock), when set.
    pub lockfile: Option<&'a str>,
    pub manifests: Vec<&'a Str>,
    pub line: usize,
}

/// How a `maven.install` tag sets `fail_if_repin_required`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepinPolicy {
    /// Not set, so the rules_jvm_external default (true) applies.
    Unset,
    True,
    False,
    Computed,
}

/// One rules_jvm_external `maven.install` tag: a hub and its literal lock file label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MavenInstall<'a> {
    pub name: &'a str,
    pub lock_file: Option<&'a str>,
    pub fail_if_repin_required: RepinPolicy,
    pub line: usize,
}

/// One aspect_rules_js `npm.npm_translate_lock` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NpmLock<'a> {
    pub name: &'a str,
    pub pnpm_lock: Option<&'a str>,
    pub line: usize,
}

/// One rules_py `uv.project` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UvProject<'a> {
    pub hub_name: &'a str,
    pub pyproject: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One gazelle `go_deps.from_file` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoModFile<'a> {
    pub go_mod: &'a str,
    pub line: usize,
}

/// One `ruby.bundle_fetch` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleFetch<'a> {
    pub name: &'a str,
    pub gemfile: &'a str,
    pub gemfile_lock: &'a str,
    pub line: usize,
}

/// One `cpan.install` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpanInstall<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `from_manifest` tag of the rules_julia `pkg` extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JuliaManifest<'a> {
    pub name: &'a str,
    pub lockfile: &'a str,
    pub line: usize,
}

/// One `composer.lock` tag of the rules_dx composer extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposerLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub digests: &'a str,
    pub line: usize,
}

/// One `luarocks.lock` tag of the rules_dx luarocks extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaRocksLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub digests: &'a str,
    pub line: usize,
}

/// One `fortran.fpm` tag of the rules_dx Fortran extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FpmLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `zig.lock` tag of the rules_dx zig extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZigLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub digests: &'a str,
    pub line: usize,
}

/// One `dub.lock` tag of the rules_dx dub extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DubLock<'a> {
    pub name: &'a str,
    pub selections: &'a str,
    pub digests: &'a str,
    pub line: usize,
}

/// One `ansible.lock` tag of the rules_dx ansible extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnsibleCollections<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `bicep.registry_module` tag of the rules_dx bicep extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BicepRegistryModule<'a> {
    pub name: &'a str,
    pub reference: &'a str,
    pub digest: &'a str,
    pub line: usize,
}

/// One `ocaml.lock` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpamPackages<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `from_package` tag of the rules_swift_package_manager `swift_deps` extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwiftPackage<'a> {
    pub swift: &'a str,
    pub resolved: &'a str,
    pub line: usize,
}

/// One `haskell.lock` tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HaskellPackages<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `pub.from_lock` tag of rules_dart or `flutter.pub` tag of rules_flutter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PubLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `elixir.lock` tag of rules_dx or this repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// One `erlang.rebar` tag of rules_dx or this repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebarLock<'a> {
    pub name: &'a str,
    pub lock: &'a str,
    pub line: usize,
}

/// A parsed MODULE.bazel with its top-level calls in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFile {
    source: String,
    calls: Vec<Call>,
}

const MAVEN_MODULE: &str = "rules_jvm_external";
const RUST_MODULE: &str = "rules_rust";
const JS_MODULE: &str = "aspect_rules_js";
const PY_MODULE: &str = "rules_py";
const GAZELLE_MODULE: &str = "gazelle";
const RUBY_MODULE: &str = "rules_dx";
const PERL_MODULE: &str = "rules_perl";
const DART_MODULE: &str = "rules_dart";
const FLUTTER_MODULE: &str = "rules_flutter";
const JULIA_MODULE: &str = "rules_julia";
const SWIFT_PM_MODULE: &str = "rules_swift_package_manager";

fn dialect() -> Dialect {
    Dialect {
        enable_def: false,
        enable_lambda: false,
        enable_load: false,
        ..Dialect::Standard
    }
}

impl ModuleFile {
    /// Parses MODULE.bazel source without evaluating it.
    pub fn parse(source: &str) -> Result<ModuleFile, ModuleError> {
        Self::parse_with(source, &dialect())
    }

    /// Parses a BUILD file the same way, accepting `load` statements and ignoring them.
    pub fn parse_build(source: &str) -> Result<ModuleFile, ModuleError> {
        let build = Dialect {
            enable_load: true,
            ..dialect()
        };
        Self::parse_with(source, &build)
    }

    fn parse_with(source: &str, dialect: &Dialect) -> Result<ModuleFile, ModuleError> {
        let ast =
            AstModule::parse("MODULE.bazel", source.to_owned(), dialect).map_err(|error| {
                ModuleError::Syntax {
                    line: error
                        .span()
                        .map_or(1, |span| span.resolve_span().begin.line + 1),
                    message: error.without_diagnostic().to_string(),
                }
            })?;
        let mut calls = Vec::new();
        collect_calls(&ast, ast.statement(), &mut calls);
        Ok(ModuleFile {
            source: source.to_owned(),
            calls,
        })
    }

    /// The source the file was parsed from.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Every top-level call in source order.
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    /// Top-level calls with the given name.
    pub fn calls_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Call> + 'a {
        self.calls.iter().filter(move |call| call.name == name)
    }

    /// The single `bazel_dep` that declares module `name`.
    pub fn bazel_dep(&self, name: &str) -> Result<&Call, ModuleError> {
        let mut found = Vec::new();
        for call in self.calls_named("bazel_dep") {
            match call.arg("name") {
                Some(Value::Str(literal)) => {
                    if literal.value == name {
                        found.push(call);
                    }
                }
                _ => return Err(ModuleError::ComputedName { line: call.line }),
            }
        }
        match found.as_slice() {
            [] => Err(ModuleError::NotFound {
                name: name.to_owned(),
            }),
            [call] => Ok(call),
            many => Err(ModuleError::Ambiguous {
                name: name.to_owned(),
                count: many.len(),
            }),
        }
    }

    /// The literal version token of the single `bazel_dep` for `name`.
    pub fn bazel_dep_version(&self, name: &str) -> Result<&Str, ModuleError> {
        let call = self.bazel_dep(name)?;
        match call.arg("version") {
            Some(Value::Str(literal)) => Ok(literal),
            Some(_) => Err(ModuleError::ComputedVersion {
                name: name.to_owned(),
                line: call.line,
            }),
            None if call.unpacked => Err(ModuleError::ComputedVersion {
                name: name.to_owned(),
                line: call.line,
            }),
            None => Err(ModuleError::MissingVersion {
                name: name.to_owned(),
                line: call.line,
            }),
        }
    }

    /// The source with the `bazel_dep` version of `name` set to `version`.
    pub fn set_bazel_dep_version(&self, name: &str, version: &str) -> Result<String, ModuleError> {
        let literal = self.bazel_dep_version(name)?;
        self.replace(literal, version)
    }

    /// The single `single_version_override` whose `module_name` is `name`.
    pub fn single_version_override(&self, name: &str) -> Result<&Call, ModuleError> {
        let mut found = Vec::new();
        for call in self.calls_named("single_version_override") {
            match call.arg("module_name") {
                Some(Value::Str(literal)) => {
                    if literal.value == name {
                        found.push(call);
                    }
                }
                _ => return Err(ModuleError::OverrideComputedName { line: call.line }),
            }
        }
        match found.as_slice() {
            [] => Err(ModuleError::OverrideNotFound {
                name: name.to_owned(),
            }),
            [call] => Ok(call),
            many => Err(ModuleError::OverrideAmbiguous {
                name: name.to_owned(),
                count: many.len(),
            }),
        }
    }

    /// The literal `version` token of the single `single_version_override` for `name`.
    pub fn override_version(&self, name: &str) -> Result<&Str, ModuleError> {
        let call = self.single_version_override(name)?;
        match call.arg("version") {
            Some(Value::Str(literal)) => Ok(literal),
            Some(_) => Err(ModuleError::OverrideComputedVersion {
                name: name.to_owned(),
                line: call.line,
            }),
            None if call.unpacked => Err(ModuleError::OverrideComputedVersion {
                name: name.to_owned(),
                line: call.line,
            }),
            None => Err(ModuleError::OverrideMissingVersion {
                name: name.to_owned(),
                line: call.line,
            }),
        }
    }

    /// The source with the `single_version_override` version of `name` set to `version`,
    /// keeping its patches and every other byte.
    pub fn set_override_version(&self, name: &str, version: &str) -> Result<String, ModuleError> {
        let literal = self.override_version(name)?;
        self.replace(literal, version)
    }

    /// The reason of the `# floor: ` comment lines directly above the `bazel_dep` of `name`,
    /// joined by spaces, or None when there is none.
    pub fn floor_comment(&self, name: &str) -> Result<Option<String>, ModuleError> {
        let call = self.bazel_dep(name)?;
        let (_, lines) = self.floor_lines(call)?;
        if lines.is_empty() {
            return Ok(None);
        }
        let reasons: Vec<&str> = lines
            .iter()
            .map(|line| line.trim_start()[FLOOR_MARKER.len()..].trim())
            .collect();
        Ok(Some(reasons.join(" ")))
    }

    /// The source with the contiguous `# floor: ` lines directly above the `bazel_dep` of
    /// `name` replaced by one `# floor: <reason>` line, or that line inserted when there is
    /// none. Other comments and every other byte are kept.
    pub fn set_floor_comment(&self, name: &str, reason: &str) -> Result<String, ModuleError> {
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().any(char::is_control) {
            return Err(ModuleError::UnsafeComment {
                value: reason.to_owned(),
            });
        }
        let call = self.bazel_dep(name)?;
        let (start, _) = self.floor_lines(call)?;
        let line_start = self.source[..call.span.start]
            .rfind('\n')
            .map_or(0, |at| at + 1);
        let indent = &self.source[line_start..call.span.start];
        let eol = line_ending(&self.source);
        let text = format!("{indent}{FLOOR_MARKER}{reason}{eol}");
        let mut out = String::with_capacity(self.source.len() + text.len());
        out.push_str(&self.source[..start]);
        out.push_str(&text);
        out.push_str(&self.source[line_start..]);
        let reparsed = ModuleFile::parse(&out)?;
        if reparsed.floor_comment(name)?.as_deref() != Some(reason) {
            return Err(ModuleError::NoInsertPoint {
                reason: "floor comment did not round-trip".to_owned(),
            });
        }
        Ok(out)
    }

    /// The byte offset where the `# floor: ` lines above `call` start, and those lines.
    fn floor_lines(&self, call: &Call) -> Result<(usize, Vec<&str>), ModuleError> {
        let line_start = self.source[..call.span.start]
            .rfind('\n')
            .map_or(0, |at| at + 1);
        if !self.source[line_start..call.span.start].trim().is_empty() {
            return Err(ModuleError::NoInsertPoint {
                reason: format!(
                    "bazel_dep at line {} does not start its own line",
                    call.line
                ),
            });
        }
        let mut start = line_start;
        let mut lines = Vec::new();
        while start > 0 {
            let prev_end = start - 1;
            let prev_start = self.source[..prev_end].rfind('\n').map_or(0, |at| at + 1);
            let line = self.source[prev_start..prev_end].trim_end_matches('\r');
            if !line.trim_start().starts_with(FLOOR_MARKER) {
                break;
            }
            lines.push(line);
            start = prev_start;
        }
        lines.reverse();
        Ok((start, lines))
    }

    /// The repository name module `name` is visible as, if it is declared.
    pub fn repo_name(&self, name: &str) -> Option<&str> {
        self.calls_named("bazel_dep")
            .find(|call| call.str("name") == Some(name))
            .and_then(|call| call.str("repo_name").or_else(|| call.str("name")))
    }

    /// The `use_extension` calls with literal arguments, as `(bzl_file label, extension name)`.
    pub fn extensions(&self) -> Vec<(&str, &str)> {
        self.calls_named("use_extension")
            .filter_map(|call| {
                Some((
                    extension_arg(call, 0, "extension_bzl_file")?,
                    extension_arg(call, 1, "extension_name")?,
                ))
            })
            .collect()
    }

    /// The variables bound to `use_extension(bzl, name)` calls, with their line.
    pub fn extension_bindings(&self, bzl: &str, name: &str) -> Vec<(&str, usize)> {
        self.calls_named("use_extension")
            .filter_map(|call| {
                if extension_arg(call, 0, "extension_bzl_file")? != bzl
                    || extension_arg(call, 1, "extension_name")? != name
                {
                    return None;
                }
                Some((call.binding.as_deref()?, call.line))
            })
            .collect()
    }

    /// The literal repo names `use_repo(binding, ...)` imports, with the call's line.
    pub fn use_repos(&self, binding: &str) -> Vec<(&str, usize)> {
        let mut out = Vec::new();
        for call in self.calls_named("use_repo") {
            if !matches!(call.positional.first(), Some(Value::Ident(v)) if v == binding) {
                continue;
            }
            for value in &call.positional[1..] {
                if let Value::Str(repo) = value {
                    out.push((repo.value.as_str(), call.line));
                }
            }
        }
        out
    }

    /// Every literal coordinate in the `artifacts` of rules_jvm_external `maven.install` tags.
    pub fn maven_artifacts(&self) -> Result<Vec<&Str>, ModuleError> {
        self.maven_artifacts_matching(None)
    }

    /// The literal coordinates in the `artifacts` of the `maven.install` named `install`
    /// only (`maven` when the tag has no name). A computed name is an error.
    pub fn maven_artifacts_of(&self, install: &str) -> Result<Vec<&Str>, ModuleError> {
        self.maven_artifacts_matching(Some(install))
    }

    fn maven_artifacts_matching(&self, install: Option<&str>) -> Result<Vec<&Str>, ModuleError> {
        let proxies = self.maven_proxies();
        let mut artifacts = Vec::new();
        for call in &self.calls {
            let Some((proxy, "install")) = call.name.rsplit_once('.') else {
                continue;
            };
            if !proxies.contains(&proxy) {
                continue;
            }
            if let Some(wanted) = install {
                let computed = ModuleError::ComputedMavenHub { line: call.line };
                if call.unpacked {
                    return Err(computed);
                }
                let name = match call.arg("name") {
                    None => "maven",
                    Some(Value::Str(literal)) => literal.value.as_str(),
                    Some(_) => return Err(computed),
                };
                if name != wanted {
                    continue;
                }
            }
            match call.arg("artifacts") {
                None if !call.unpacked => {}
                Some(Value::List(items)) => {
                    for item in items {
                        match item {
                            Value::Str(literal) => artifacts.push(literal),
                            _ => return Err(ModuleError::ComputedArtifacts { line: call.line }),
                        }
                    }
                }
                _ => return Err(ModuleError::ComputedArtifacts { line: call.line }),
            }
        }
        Ok(artifacts)
    }

    /// Every rules_rust `crate.from_cargo` tag in source order, with its literal manifests.
    pub fn cargo_hubs(&self) -> Result<Vec<CargoHub<'_>>, ModuleError> {
        let proxies = self.crate_proxies();
        let mut hubs = Vec::new();
        for call in &self.calls {
            let Some((proxy, "from_cargo")) = call.name.rsplit_once('.') else {
                continue;
            };
            if !proxies.contains(&proxy) {
                continue;
            }
            let computed = ModuleError::ComputedCargoHub { line: call.line };
            if call.unpacked {
                return Err(computed);
            }
            let Some(Value::Str(name)) = call.arg("name") else {
                return Err(computed);
            };
            let cargo_lockfile = match call.arg("cargo_lockfile") {
                None => None,
                Some(Value::Str(literal)) => Some(literal.value.as_str()),
                Some(_) => return Err(computed),
            };
            let lockfile = match call.arg("lockfile") {
                None => None,
                Some(Value::Str(literal)) => Some(literal.value.as_str()),
                Some(_) => return Err(computed),
            };
            let mut manifests = Vec::new();
            match call.arg("manifests") {
                None => {}
                Some(Value::List(items)) => {
                    for item in items {
                        match item {
                            Value::Str(literal) => manifests.push(literal),
                            _ => return Err(computed),
                        }
                    }
                }
                Some(_) => return Err(computed),
            }
            hubs.push(CargoHub {
                name: &name.value,
                cargo_lockfile,
                lockfile,
                manifests,
                line: call.line,
            });
        }
        Ok(hubs)
    }

    /// The name a `from_cargo` hub is visible under in the root module: `x` for
    /// `use_repo(crate, "x")`, `alias` for `use_repo(crate, alias = "x")`. None when
    /// the hub is not imported. Bazel resolves `@<name>` in the root module by this
    /// visible name, while crate_universe compares `CARGO_BAZEL_REPIN_ONLY` with the
    /// hub's own name.
    pub fn cargo_apparent(&self, hub: &CargoHub<'_>) -> Option<&str> {
        let proxies = self.crate_proxies();
        for call in self.calls_named("use_repo") {
            if !matches!(call.positional.first(), Some(Value::Ident(v)) if proxies.contains(&v.as_str()))
            {
                continue;
            }
            for value in &call.positional[1..] {
                if let Value::Str(repo) = value
                    && repo.value == hub.name
                {
                    return Some(repo.value.as_str());
                }
            }
            for (alias, value) in &call.named {
                if matches!(value, Value::Str(repo) if repo.value == hub.name) {
                    return Some(alias.as_str());
                }
            }
        }
        None
    }

    /// Every rules_jvm_external `maven.install` tag in source order. The name defaults
    /// to `maven`; a computed name or lock_file is an error.
    pub fn maven_installs(&self) -> Result<Vec<MavenInstall<'_>>, ModuleError> {
        let proxies = self.maven_proxies();
        let mut installs = Vec::new();
        for call in &self.calls {
            let Some((proxy, "install")) = call.name.rsplit_once('.') else {
                continue;
            };
            if !proxies.contains(&proxy) {
                continue;
            }
            let computed = ModuleError::ComputedMavenHub { line: call.line };
            if call.unpacked {
                return Err(computed);
            }
            let name = match call.arg("name") {
                None => "maven",
                Some(Value::Str(literal)) => literal.value.as_str(),
                Some(_) => return Err(computed),
            };
            let lock_file = match call.arg("lock_file") {
                None => None,
                Some(Value::Str(literal)) => Some(literal.value.as_str()),
                Some(_) => return Err(computed),
            };
            let fail_if_repin_required = match call.arg("fail_if_repin_required") {
                None => RepinPolicy::Unset,
                Some(Value::Ident(word)) if word == "True" => RepinPolicy::True,
                Some(Value::Ident(word)) if word == "False" => RepinPolicy::False,
                Some(_) => RepinPolicy::Computed,
            };
            installs.push(MavenInstall {
                name,
                lock_file,
                fail_if_repin_required,
                line: call.line,
            });
        }
        Ok(installs)
    }

    /// Every aspect_rules_js `npm.npm_translate_lock` tag in source order.
    pub fn npm_translate_locks(&self) -> Result<Vec<NpmLock<'_>>, ModuleError> {
        const TAG: &str = "npm.npm_translate_lock";
        self.tags(JS_MODULE, "npm:extensions.bzl", "npm", "npm_translate_lock")
            .map(|call| {
                Ok(NpmLock {
                    name: required(call, TAG, "name")?,
                    pnpm_lock: literal(call, TAG, "pnpm_lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every rules_py `uv.project` tag in source order.
    pub fn uv_projects(&self) -> Result<Vec<UvProject<'_>>, ModuleError> {
        const TAG: &str = "uv.project";
        self.tags(PY_MODULE, "uv:extensions.bzl", "uv", "project")
            .map(|call| {
                Ok(UvProject {
                    hub_name: required(call, TAG, "hub_name")?,
                    pyproject: required(call, TAG, "pyproject")?,
                    lock: required(call, TAG, "lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every gazelle `go_deps.from_file` tag that names a `go_mod`, in source order.
    pub fn go_deps_files(&self) -> Result<Vec<GoModFile<'_>>, ModuleError> {
        const TAG: &str = "go_deps.from_file";
        let mut out = Vec::new();
        for call in self.tags(GAZELLE_MODULE, ":extensions.bzl", "go_deps", "from_file") {
            if let Some(go_mod) = literal(call, TAG, "go_mod")? {
                out.push(GoModFile {
                    go_mod,
                    line: call.line,
                });
            }
        }
        Ok(out)
    }

    /// Every `ruby.bundle_fetch` tag in source order. The name defaults to `bundle`.
    pub fn bundle_fetches(&self) -> Result<Vec<BundleFetch<'_>>, ModuleError> {
        const TAG: &str = "ruby.bundle_fetch";
        self.tags(RUBY_MODULE, "ruby:extensions.bzl", "ruby", "bundle_fetch")
            .map(|call| {
                Ok(BundleFetch {
                    name: literal(call, TAG, "name")?.unwrap_or("bundle"),
                    gemfile: required(call, TAG, "gemfile")?,
                    gemfile_lock: required(call, TAG, "gemfile_lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `pub.from_lock` tag of rules_dart or this repository's `//dart:pub.bzl` and
    /// `flutter.pub` tag of rules_flutter, in source order within each extension.
    pub fn pub_locks(&self) -> Result<Vec<PubLock<'_>>, ModuleError> {
        let dart = self
            .tags(DART_MODULE, "dart/pub:extensions.bzl", "pub", "from_lock")
            .map(|call| ("pub.from_lock", call));
        let flutter = self
            .tags(FLUTTER_MODULE, "flutter:extensions.bzl", "flutter", "pub")
            .map(|call| ("flutter.pub", call));
        let own = self
            .tags_of("//dart:pub.bzl".to_owned(), "pub", "from_lock")
            .map(|call| ("pub.from_lock", call));
        dart.chain(flutter)
            .chain(own)
            .map(|(tag, call)| {
                Ok(PubLock {
                    name: required(call, tag, "name")?,
                    lock: required(call, tag, "lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `elixir.lock` tag of rules_dx or this repository's `//elixir:extensions.bzl`.
    pub fn mix_locks(&self) -> Result<Vec<MixLock<'_>>, ModuleError> {
        const TAG: &str = "elixir.lock";
        let external = self.tags(
            RUBY_MODULE,
            "elixir:extensions.bzl",
            "elixir",
            "lock",
        );
        let own = self.tags_of(
            "//elixir:extensions.bzl".to_owned(),
            "elixir",
            "lock",
        );
        external
            .chain(own)
            .map(|call| {
                Ok(MixLock {
                    name: required(call, TAG, "name")?,
                    lock: required(call, TAG, "lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `erlang.rebar` tag of rules_dx or this repository's `//erlang:extensions.bzl`.
    pub fn rebar_locks(&self) -> Result<Vec<RebarLock<'_>>, ModuleError> {
        const TAG: &str = "erlang.rebar";
        let external = self.tags(RUBY_MODULE, "erlang:extensions.bzl", "erlang", "rebar");
        let own = self.tags_of("//erlang:extensions.bzl".to_owned(), "erlang", "rebar");
        external
            .chain(own)
            .map(|call| {
                Ok(RebarLock {
                    name: required(call, TAG, "name")?,
                    lock: required(call, TAG, "lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `composer.lock` tag of the rules_dx composer extension in source order.
    pub fn composer_locks(&self) -> Result<Vec<ComposerLock<'_>>, ModuleError> {
        const TAG: &str = "composer.lock";
        self.tags(RUBY_MODULE, "php:extensions.bzl", "composer", "lock")
        .map(|call| {
            Ok(ComposerLock {
                name: required(call, TAG, "name")?,
                lock: required(call, TAG, "lock")?,
                digests: required(call, TAG, "digests")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `luarocks.lock` tag of the rules_dx luarocks extension in source order.
    pub fn luarocks_locks(&self) -> Result<Vec<LuaRocksLock<'_>>, ModuleError> {
        const TAG: &str = "luarocks.lock";
        self.tags(
            RUBY_MODULE,
            "lua/luarocks:extensions.bzl",
            "luarocks",
            "lock",
        )
        .map(|call| {
            Ok(LuaRocksLock {
                name: required(call, TAG, "name")?,
                lock: required(call, TAG, "lock")?,
                digests: required(call, TAG, "digests")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `from_manifest` tag of the rules_julia `pkg` extension in source order.
    pub fn julia_manifests(&self) -> Result<Vec<JuliaManifest<'_>>, ModuleError> {
        const TAG: &str = "julia_pkg.from_manifest";
        self.tags(
            JULIA_MODULE,
            "julia/pkg:extensions.bzl",
            "pkg",
            "from_manifest",
        )
        .map(|call| {
            Ok(JuliaManifest {
                name: required(call, TAG, "name")?,
                lockfile: required(call, TAG, "lockfile")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `from_package` tag of the rules_swift_package_manager `swift_deps` extension in source order.
    pub fn swift_packages(&self) -> Result<Vec<SwiftPackage<'_>>, ModuleError> {
        const TAG: &str = "swift_deps.from_package";
        self.tags(
            SWIFT_PM_MODULE,
            ":extensions.bzl",
            "swift_deps",
            "from_package",
        )
        .map(|call| {
            Ok(SwiftPackage {
                swift: required(call, TAG, "swift")?,
                resolved: required(call, TAG, "resolved")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `cpan.install` tag of the rules_perl extension in source order.
    pub fn cpan_installs(&self) -> Result<Vec<CpanInstall<'_>>, ModuleError> {
        const TAG: &str = "cpan.install";
        self.tags(PERL_MODULE, "perl:extensions.bzl", "cpan", "install")
            .map(|call| {
                Ok(CpanInstall {
                    name: required(call, TAG, "name")?,
                    lock: required(call, TAG, "lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `fortran.fpm` tag of the rules_dx Fortran extension in source order.
    pub fn fpm_locks(&self) -> Result<Vec<FpmLock<'_>>, ModuleError> {
        const TAG: &str = "fortran.fpm";
        let own = self.tags_of("//fortran:extensions.bzl".to_owned(), "fortran", "fpm");
        self.tags(RUBY_MODULE, "fortran:extensions.bzl", "fortran", "fpm")
            .chain(own)
            .map(|call| {
                Ok(FpmLock {
                    name: required(call, TAG, "name")?,
                    lock: required(call, TAG, "lock")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `dub.lock` tag of the rules_dx dub extension in source order.
    pub fn dub_locks(&self) -> Result<Vec<DubLock<'_>>, ModuleError> {
        const TAG: &str = "dub.lock";
        let own = self.tags_of("//d:extensions.bzl".to_owned(), "dub", "lock");
        self.tags(RUBY_MODULE, "d:extensions.bzl", "dub", "lock")
            .chain(own)
            .map(|call| {
                Ok(DubLock {
                    name: required(call, TAG, "name")?,
                    selections: required(call, TAG, "lock")?,
                    digests: required(call, TAG, "digests")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `zig.lock` tag of the rules_dx zig deps extension in source order.
    pub fn zig_locks(&self) -> Result<Vec<ZigLock<'_>>, ModuleError> {
        const TAG: &str = "zig.lock";
        let own = self.tags_of("//zig:extensions.bzl".to_owned(), "zig_deps", "lock");
        self.tags(RUBY_MODULE, "zig:extensions.bzl", "zig_deps", "lock")
            .chain(own)
            .map(|call| {
                Ok(ZigLock {
                    name: required(call, TAG, "name")?,
                    lock: required(call, TAG, "lock")?,
                    digests: required(call, TAG, "digests")?,
                    line: call.line,
                })
            })
            .collect()
    }

    /// Every `ansible.lock` tag of the rules_dx ansible extension in source order.
    pub fn ansible_collections(&self) -> Result<Vec<AnsibleCollections<'_>>, ModuleError> {
        const TAG: &str = "ansible.lock";
        let own = self.tags_of("//ansible:extensions.bzl".to_owned(), "ansible", "lock");
        self.tags(
            RUBY_MODULE,
            "ansible:extensions.bzl",
            "ansible",
            "collections",
        )
        .chain(own)
        .map(|call| {
            Ok(AnsibleCollections {
                name: required(call, TAG, "name")?,
                lock: required(call, TAG, "lock")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `bicep.registry_module` tag of the rules_dx bicep extension in source order.
    pub fn bicep_registry_modules(&self) -> Result<Vec<BicepRegistryModule<'_>>, ModuleError> {
        const TAG: &str = "bicep.registry_module";
        let own = self.tags_of(
            "//bicep:extensions.bzl".to_owned(),
            "bicep",
            "registry_module",
        );
        self.tags(
            RUBY_MODULE,
            "bicep:extensions.bzl",
            "bicep",
            "registry_module",
        )
        .chain(own)
        .map(|call| {
            Ok(BicepRegistryModule {
                name: required(call, TAG, "name")?,
                reference: required(call, TAG, "ref")?,
                digest: required(call, TAG, "digest")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `ocaml.lock` tag of this repository's OCaml extension in source order.
    pub fn opam_packages(&self) -> Result<Vec<OpamPackages<'_>>, ModuleError> {
        const TAG: &str = "ocaml.lock";
        let own = self.tags_of(
            "//ocaml/toolchain:extensions.bzl".to_owned(),
            "ocaml",
            "lock",
        );
        self.tags(
            RUBY_MODULE,
            "ocaml/toolchain:extensions.bzl",
            "ocaml",
            "lock",
        )
        .chain(own)
        .map(|call| {
            Ok(OpamPackages {
                name: required(call, TAG, "name")?,
                lock: required(call, TAG, "lock")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// Every `haskell.lock` tag of this repository's Haskell extension in source order.
    pub fn haskell_packages(&self) -> Result<Vec<HaskellPackages<'_>>, ModuleError> {
        const TAG: &str = "haskell.lock";
        let own = self.tags_of(
            "//haskell/toolchain:extensions.bzl".to_owned(),
            "haskell",
            "lock",
        );
        self.tags(
            RUBY_MODULE,
            "haskell/toolchain:extensions.bzl",
            "haskell",
            "lock",
        )
        .chain(own)
        .map(|call| {
            Ok(HaskellPackages {
                name: required(call, TAG, "name")?,
                lock: required(call, TAG, "lock")?,
                line: call.line,
            })
        })
        .collect()
    }

    /// The main-repository .bzl labels of `use_extension` calls that load paket groups:
    /// an extension named `paket`, or a generated `paket.<group>_extension.bzl` file.
    pub fn paket_extensions(&self) -> Vec<&str> {
        self.extensions()
            .into_iter()
            .filter(|(label, name)| {
                let file = label.rsplit([':', '/']).next().unwrap_or(label);
                label.starts_with("//")
                    && (*name == "paket"
                        || (file.starts_with("paket.") && file.ends_with("_extension.bzl")))
            })
            .map(|(label, _)| label)
            .collect()
    }

    /// The main-repository .bzl labels of `use_extension` calls that load Conan's `BazelDeps` output.
    pub fn conan_extensions(&self) -> Vec<&str> {
        self.extensions()
            .into_iter()
            .filter(|(label, _)| {
                let file = label.rsplit([':', '/']).next().unwrap_or(label);
                label.starts_with("//") && file == "conan_deps_module_extension.bzl"
            })
            .map(|(label, _)| label)
            .collect()
    }

    /// The calls of `tag` on every binding of the extension `name` in `@<module>//<file>`.
    fn tags<'a>(
        &'a self,
        module: &str,
        file: &str,
        name: &str,
        tag: &'a str,
    ) -> impl Iterator<Item = &'a Call> + 'a {
        let repo = self.repo_name(module).unwrap_or(module);
        self.tags_of(format!("@{repo}//{file}"), name, tag)
    }

    fn tags_of<'a>(
        &'a self,
        label: String,
        name: &str,
        tag: &'a str,
    ) -> impl Iterator<Item = &'a Call> + 'a {
        let proxies: Vec<&str> = self
            .calls_named("use_extension")
            .filter(|call| {
                extension_arg(call, 0, "extension_bzl_file") == Some(label.as_str())
                    && extension_arg(call, 1, "extension_name") == Some(name)
            })
            .filter_map(|call| call.binding.as_deref())
            .collect();
        self.calls.iter().filter(move |call| {
            matches!(call.name.rsplit_once('.'), Some((proxy, t)) if t == tag && proxies.contains(&proxy))
        })
    }

    /// The source with `bazel_dep(name = name, version = version)` added after the
    /// last top-level `bazel_dep`, or after `module()`, keeping every other byte.
    pub fn insert_bazel_dep(&self, name: &str, version: &str) -> Result<String, ModuleError> {
        for value in [name, version] {
            check_safe(value)?;
        }
        match self.bazel_dep(name) {
            Ok(_) | Err(ModuleError::Ambiguous { .. }) => {
                return Err(ModuleError::AlreadyDeclared {
                    name: name.to_owned(),
                });
            }
            Err(ModuleError::NotFound { .. }) => {}
            Err(other) => return Err(other),
        }
        let eol = line_ending(&self.source);
        let line = format!("bazel_dep(name = \"{name}\", version = \"{version}\")");
        let anchor = self
            .calls_named("bazel_dep")
            .last()
            .map(|call| (call.span.end, false))
            .or_else(|| {
                self.calls_named("module")
                    .last()
                    .map(|call| (call.span.end, true))
            });
        let out = match anchor {
            Some((end, after_module)) => {
                let at = line_end(&self.source, end);
                let mut text = String::new();
                if at == self.source.len() && !self.source.ends_with('\n') {
                    text.push_str(eol);
                }
                if after_module {
                    text.push_str(eol);
                }
                text.push_str(&line);
                text.push_str(eol);
                splice(&self.source, at, &text)
            }
            None => {
                let mut text = String::new();
                if !self.source.is_empty() && !self.source.ends_with('\n') {
                    text.push_str(eol);
                }
                if !self.source.trim().is_empty() {
                    text.push_str(eol);
                }
                text.push_str(&line);
                text.push_str(eol);
                splice(&self.source, self.source.len(), &text)
            }
        };
        let reparsed = ModuleFile::parse(&out)?;
        if reparsed.bazel_dep_version(name).map(|v| v.value.as_str()) != Ok(version) {
            return Err(ModuleError::NoInsertPoint {
                reason: "insert did not round-trip".to_owned(),
            });
        }
        Ok(out)
    }

    /// The source with a block appended at the end: `binding = use_extension(bzl, ext)` (skipped
    /// when `binding` is already bound to that extension), the `tags`, and a `use_repo` for the
    /// `repos` not yet imported through `binding`. Every other byte is kept.
    pub fn insert_extension_block(
        &self,
        binding: &str,
        bzl: &str,
        ext: &str,
        tags: &[NewTag],
        repos: &[&str],
    ) -> Result<String, ModuleError> {
        for value in [binding, bzl, ext].into_iter().chain(repos.iter().copied()) {
            check_safe(value)?;
        }
        for tag in tags {
            tag.check()?;
        }
        let bound = self
            .extension_bindings(bzl, ext)
            .iter()
            .any(|(name, _)| *name == binding);
        let imported = self.use_repos(binding);
        let missing: Vec<&str> = repos
            .iter()
            .copied()
            .filter(|repo| !imported.iter().any(|(have, _)| have == repo))
            .collect();
        let eol = line_ending(&self.source);
        let mut text = String::new();
        if !self.source.is_empty() && !self.source.ends_with('\n') {
            text.push_str(eol);
        }
        if !self.source.trim().is_empty() {
            text.push_str(eol);
        }
        let mut lines = Vec::new();
        if !bound {
            lines.push(format!("{binding} = use_extension(\"{bzl}\", \"{ext}\")"));
        }
        lines.extend(tags.iter().map(|tag| tag.render(binding)));
        if !missing.is_empty() {
            let quoted: Vec<String> = missing.iter().map(|repo| format!("\"{repo}\"")).collect();
            lines.push(format!("use_repo({binding}, {})", quoted.join(", ")));
        }
        for line in lines {
            for part in line.split('\n') {
                text.push_str(part);
                text.push_str(eol);
            }
        }
        let out = splice(&self.source, self.source.len(), &text);
        let reparsed = ModuleFile::parse(&out)?;
        let tag_name = |tag: &NewTag| format!("{binding}.{}", tag.name);
        let count = |file: &ModuleFile, name: &str| file.calls_named(name).count();
        let round_trips = tags.iter().all(|tag| {
            let name = tag_name(tag);
            count(&reparsed, &name) > count(self, &name)
        });
        if !round_trips {
            return Err(ModuleError::NoInsertPoint {
                reason: "insert did not round-trip".to_owned(),
            });
        }
        Ok(out)
    }

    /// The `include()` labels of this file, in source order.
    pub fn include_labels(&self) -> Vec<&str> {
        self.calls_named("include")
            .filter_map(
                |call| match call.positional.first().or_else(|| call.arg("label")) {
                    Some(Value::Str(literal)) => Some(literal.value.as_str()),
                    _ => None,
                },
            )
            .collect()
    }

    /// The source with `include("label")` added on its own line after the `bazel_dep` of
    /// `after_dep`, else after the last `bazel_dep`, else after `module()`, else at the end.
    /// Returns the source unchanged when `label` is already included.
    pub fn insert_include(&self, label: &str, after_dep: &str) -> Result<String, ModuleError> {
        check_safe(label)?;
        if self.include_labels().contains(&label) {
            return Ok(self.source.clone());
        }
        let eol = line_ending(&self.source);
        let line = format!("include(\"{label}\")");
        let anchor = self
            .calls_named("bazel_dep")
            .find(|call| call.str("name") == Some(after_dep))
            .or_else(|| self.calls_named("bazel_dep").last())
            .map(|call| (call.span.end, false))
            .or_else(|| {
                self.calls_named("module")
                    .last()
                    .map(|call| (call.span.end, true))
            });
        let (at, mut text) = match anchor {
            Some((end, after_module)) => {
                let at = line_end(&self.source, end);
                let mut text = String::new();
                if at == self.source.len() && !self.source.ends_with('\n') {
                    text.push_str(eol);
                }
                if after_module {
                    text.push_str(eol);
                }
                (at, text)
            }
            None => {
                let mut text = String::new();
                if !self.source.is_empty() && !self.source.ends_with('\n') {
                    text.push_str(eol);
                }
                if !self.source.trim().is_empty() {
                    text.push_str(eol);
                }
                (self.source.len(), text)
            }
        };
        text.push_str(&line);
        text.push_str(eol);
        let out = splice(&self.source, at, &text);
        let reparsed = ModuleFile::parse(&out)?;
        if !reparsed.include_labels().contains(&label) {
            return Err(ModuleError::NoInsertPoint {
                reason: "insert did not round-trip".to_owned(),
            });
        }
        Ok(out)
    }

    /// The source with Maven coordinate `coordinate` appended to the `artifacts` list of the
    /// one rules_jvm_external `maven.install` tag that feeds the default `@maven` repository.
    pub fn insert_maven_artifact(&self, coordinate: &str) -> Result<String, ModuleError> {
        self.insert_maven_artifact_in("maven", coordinate)
    }

    /// Like [`ModuleFile::insert_maven_artifact`], for the `maven.install(name = <repository>)`
    /// tag that feeds `use_repo(maven, "<repository>")`.
    pub fn insert_maven_artifact_in(
        &self,
        repository: &str,
        coordinate: &str,
    ) -> Result<String, ModuleError> {
        check_safe(coordinate)?;
        let proxies = self.maven_proxies();
        let mut installs: Vec<&Call> = Vec::new();
        for call in &self.calls {
            let is_install = call
                .name
                .rsplit_once('.')
                .is_some_and(|(proxy, tag)| tag == "install" && proxies.contains(&proxy));
            if !is_install {
                continue;
            }
            match call.arg("name") {
                None if call.unpacked => {
                    return Err(ModuleError::NoInsertPoint {
                        reason: format!("maven.install at line {} has a computed name", call.line),
                    });
                }
                None if repository == "maven" => installs.push(call),
                None => {}
                Some(Value::Str(literal)) if literal.value == repository => installs.push(call),
                Some(Value::Str(_)) => {}
                Some(_) => {
                    return Err(ModuleError::NoInsertPoint {
                        reason: format!("maven.install at line {} has a computed name", call.line),
                    });
                }
            }
        }
        let call = match installs.as_slice() {
            [call] => *call,
            [] => {
                return Err(ModuleError::NoInsertPoint {
                    reason: format!("no maven.install tag for the @{repository} repository"),
                });
            }
            many => {
                return Err(ModuleError::NoInsertPoint {
                    reason: format!(
                        "{} maven.install tags feed the @{repository} repository",
                        many.len()
                    ),
                });
            }
        };
        let items = match call.arg("artifacts") {
            Some(Value::List(items)) => items,
            _ => return Err(ModuleError::ComputedArtifacts { line: call.line }),
        };
        let mut last = None;
        for item in items {
            match item {
                Value::Str(literal) => last = Some(literal),
                _ => return Err(ModuleError::ComputedArtifacts { line: call.line }),
            }
        }
        let Some(last) = last else {
            return Err(ModuleError::NoInsertPoint {
                reason: format!("maven.install artifacts at line {} is empty", call.line),
            });
        };
        let source = &self.source;
        let line_start = source[..last.span.start].rfind('\n').map_or(0, |at| at + 1);
        let before = &source[line_start..last.span.start];
        let quote = if source[last.span.clone()].starts_with('\'') {
            '\''
        } else {
            '"'
        };
        let literal = format!("{quote}{coordinate}{quote}");
        let out = if before.trim().is_empty() {
            let rest = &source[last.span.end..];
            let comma = rest.trim_start_matches([' ', '\t']).starts_with(',');
            let eol = line_ending(source);
            if comma {
                let at = line_end(source, last.span.end);
                let mut text = String::new();
                if at == source.len() {
                    text.push_str(eol);
                }
                text.push_str(before);
                text.push_str(&literal);
                text.push(',');
                text.push_str(eol);
                splice(source, at, &text)
            } else {
                splice(source, last.span.end, &format!(",{eol}{before}{literal}"))
            }
        } else {
            splice(source, last.span.end, &format!(", {literal}"))
        };
        let reparsed = ModuleFile::parse(&out)?;
        let mut expected: Vec<String> = self
            .maven_artifacts()?
            .iter()
            .map(|s| s.value.clone())
            .collect();
        expected.push(coordinate.to_owned());
        let after: Vec<String> = reparsed
            .maven_artifacts()?
            .iter()
            .map(|s| s.value.clone())
            .collect();
        let mut sorted_expected = expected.clone();
        sorted_expected.sort();
        let mut sorted_after = after;
        sorted_after.sort();
        if sorted_after != sorted_expected {
            return Err(ModuleError::NoInsertPoint {
                reason: "insert did not round-trip".to_owned(),
            });
        }
        Ok(out)
    }

    /// The bindings of the `use_extension` calls for rules_jvm_external's `maven`.
    pub fn maven_proxies(&self) -> Vec<&str> {
        let repo = self.repo_name(MAVEN_MODULE).unwrap_or(MAVEN_MODULE);
        let label = format!("@{repo}//:extensions.bzl");
        self.calls_named("use_extension")
            .filter(|call| {
                let file = extension_arg(call, 0, "extension_bzl_file");
                let extension = extension_arg(call, 1, "extension_name");
                file == Some(label.as_str()) && extension == Some("maven")
            })
            .filter_map(|call| call.binding.as_deref())
            .collect()
    }

    fn crate_proxies(&self) -> Vec<&str> {
        let repo = self.repo_name(RUST_MODULE).unwrap_or(RUST_MODULE);
        let label = format!("@{repo}//crate_universe:extensions.bzl");
        self.calls_named("use_extension")
            .filter(|call| {
                let file = extension_arg(call, 0, "extension_bzl_file");
                let extension = extension_arg(call, 1, "extension_name");
                file == Some(label.as_str()) && extension == Some("crate")
            })
            .filter_map(|call| call.binding.as_deref())
            .collect()
    }

    /// The source with one string literal set to `value`, keeping its quote style.
    pub fn replace(&self, literal: &Str, value: &str) -> Result<String, ModuleError> {
        if literal.value == value {
            return Ok(self.source.clone());
        }
        if value
            .chars()
            .any(|c| c == '"' || c == '\'' || c == '\\' || c.is_control())
        {
            return Err(ModuleError::UnsafeString {
                value: value.to_owned(),
            });
        }
        let token = &self.source[literal.span.clone()];
        let open = token.find(['"', '\'']).unwrap_or(0);
        let body = &token[open..];
        let quote = if body.starts_with("\"\"\"") || body.starts_with("'''") {
            &body[..3]
        } else {
            &body[..1]
        };
        let mut out = String::with_capacity(self.source.len() + value.len());
        out.push_str(&self.source[..literal.span.start]);
        out.push_str(&token[..open]);
        out.push_str(quote);
        out.push_str(value);
        out.push_str(quote);
        out.push_str(&self.source[literal.span.end..]);
        Ok(out)
    }
}

/// Compares two module versions the way Bazel orders them
/// (<https://bazel.build/external/module#version_format>): release identifiers first,
/// numeric before alphanumeric and numerically among themselves, a prerelease below its
/// release, build metadata ignored, and the empty version above every other.
pub fn compare_module_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    fn split(version: &str) -> (Vec<&str>, Option<Vec<&str>>) {
        let version = version.split_once('+').map_or(version, |(head, _)| head);
        let (release, pre) = match version.split_once('-') {
            Some((release, pre)) => (release, Some(pre.split('.').collect())),
            None => (version, None),
        };
        (release.split('.').collect(), pre)
    }
    fn identifier(a: &str, b: &str) -> Ordering {
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
        match (digits(a), digits(b)) {
            (true, true) => {
                let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
                a.len().cmp(&b.len()).then_with(|| a.cmp(b))
            }
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.cmp(b),
        }
    }
    fn identifiers(a: &[&str], b: &[&str]) -> Ordering {
        for (x, y) in a.iter().zip(b) {
            let order = identifier(x, y);
            if order != Ordering::Equal {
                return order;
            }
        }
        a.len().cmp(&b.len())
    }
    match (a.is_empty(), b.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        (false, false) => {}
    }
    let (release_a, pre_a) = split(a);
    let (release_b, pre_b) = split(b);
    identifiers(&release_a, &release_b).then_with(|| match (pre_a, pre_b) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(x), Some(y)) => identifiers(&x, &y),
    })
}

fn check_safe(value: &str) -> Result<(), ModuleError> {
    if value
        .chars()
        .any(|c| c == '"' || c == '\'' || c == '\\' || c.is_control())
    {
        return Err(ModuleError::UnsafeString {
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// `\r\n` when the source uses CRLF line endings, else `\n`.
fn line_ending(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// The byte just past the newline that ends the line holding `at`, or the source end.
fn line_end(source: &str, at: usize) -> usize {
    source[at..]
        .find('\n')
        .map_or(source.len(), |off| at + off + 1)
}

fn splice(source: &str, at: usize, text: &str) -> String {
    let mut out = String::with_capacity(source.len() + text.len());
    out.push_str(&source[..at]);
    out.push_str(text);
    out.push_str(&source[at..]);
    out
}

/// The module strings of every `load()` in a .bzl source, in source order.
pub fn load_labels(source: &str) -> Result<Vec<String>, ModuleError> {
    let ast =
        AstModule::parse("file.bzl", source.to_owned(), &Dialect::Extended).map_err(|error| {
            ModuleError::StarlarkSyntax {
                line: error
                    .span()
                    .map_or(1, |span| span.resolve_span().begin.line + 1),
                message: error.without_diagnostic().to_string(),
            }
        })?;
    Ok(ast
        .loads()
        .into_iter()
        .map(|load| load.module_id.to_owned())
        .collect())
}

fn literal<'a>(
    call: &'a Call,
    tag: &'static str,
    key: &str,
) -> Result<Option<&'a str>, ModuleError> {
    match call.arg(key) {
        None if !call.unpacked => Ok(None),
        Some(Value::Str(literal)) => Ok(Some(literal.value.as_str())),
        _ => Err(ModuleError::ComputedHub {
            tag,
            line: call.line,
        }),
    }
}

fn required<'a>(call: &'a Call, tag: &'static str, key: &str) -> Result<&'a str, ModuleError> {
    literal(call, tag, key)?.ok_or(ModuleError::ComputedHub {
        tag,
        line: call.line,
    })
}

fn extension_arg<'a>(call: &'a Call, index: usize, key: &str) -> Option<&'a str> {
    match call.positional.get(index).or_else(|| call.arg(key)) {
        Some(Value::Str(literal)) => Some(&literal.value),
        _ => None,
    }
}

fn collect_calls(ast: &AstModule, stmt: &AstStmt, calls: &mut Vec<Call>) {
    match &stmt.node {
        StmtP::Statements(stmts) => {
            for stmt in stmts {
                collect_calls(ast, stmt, calls);
            }
        }
        StmtP::Expression(expr) => {
            if let Some(call) = read_call(ast, expr, None) {
                calls.push(call);
            }
        }
        StmtP::Assign(assign) => {
            let binding = match &assign.lhs.node {
                AssignTargetP::Identifier(ident) => Some(ident.node.ident.clone()),
                _ => None,
            };
            if let Some(call) = read_call(ast, &assign.rhs, binding) {
                calls.push(call);
            }
        }
        _ => {}
    }
}

fn read_call(ast: &AstModule, expr: &AstExpr, binding: Option<String>) -> Option<Call> {
    let ExprP::Call(callee, args) = &expr.node else {
        return None;
    };
    let name = callee_name(callee)?;
    let mut call = Call {
        name,
        binding,
        line: line_of(ast, expr.span),
        span: expr.span.begin().get() as usize..expr.span.end().get() as usize,
        positional: Vec::new(),
        named: Vec::new(),
        unpacked: false,
    };
    for arg in &args.args {
        match &arg.node {
            ArgumentP::Positional(value) => call.positional.push(read_value(ast, value)),
            ArgumentP::Named(key, value) => {
                call.named.push((key.node.clone(), read_value(ast, value)));
            }
            ArgumentP::Args(_) | ArgumentP::KwArgs(_) => call.unpacked = true,
        }
    }
    Some(call)
}

fn callee_name(expr: &AstExpr) -> Option<String> {
    match &expr.node {
        ExprP::Identifier(ident) => Some(ident.node.ident.clone()),
        ExprP::Dot(object, attr) => Some(format!("{}.{}", callee_name(object)?, attr.node)),
        _ => None,
    }
}

fn read_value(ast: &AstModule, expr: &AstExpr) -> Value {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(literal)) => Value::Str(Str {
            value: literal.node.clone(),
            span: literal.span.begin().get() as usize..literal.span.end().get() as usize,
        }),
        ExprP::List(items) => Value::List(items.iter().map(|item| read_value(ast, item)).collect()),
        ExprP::Dict(entries) => Value::Dict(
            entries
                .iter()
                .map(|(key, value)| (read_value(ast, key), read_value(ast, value)))
                .collect(),
        ),
        ExprP::Identifier(ident) => Value::Ident(ident.node.ident.clone()),
        _ => Value::Computed {
            line: line_of(ast, expr.span),
        },
    }
}

fn line_of(ast: &AstModule, span: Span) -> usize {
    ast.file_span(span).resolve_span().begin.line + 1
}

/// Parses the root MODULE.bazel and every file it includes, in include order.
pub fn load_with_includes(
    root_text: &str,
    read_include: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<ModuleFile>, String> {
    Ok(load_with_includes_labeled(root_text, read_include)?
        .into_iter()
        .map(|(_, file)| file)
        .collect())
}

/// Like [`load_with_includes`], pairing each file with the include label it came from
/// (`None` for the root), so a finding can name its file.
pub fn load_with_includes_labeled(
    root_text: &str,
    read_include: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<(Option<String>, ModuleFile)>, String> {
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    collect_module(root_text, None, read_include, &mut seen, &mut files)?;
    Ok(files)
}

/// Splits a main-repository `//package:file` label into package and file, or None when
/// it is not one; Err when a part would leave the workspace.
fn workspace_label_parts(label: &str) -> Option<Result<(&str, &str), ()>> {
    let (package, file) = label.strip_prefix("//")?.split_once(':')?;
    let bad =
        |part: &str| part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']);
    let package_bad = !package.is_empty() && package.split('/').any(bad);
    if package_bad || file.split('/').any(bad) {
        return Some(Err(()));
    }
    Some(Ok((package, file)))
}

/// The workspace-relative path of a main-repository `//package:file` label.
pub fn label_path(label: &str) -> Result<String, String> {
    if label.starts_with('@') {
        return Err(format!(
            "{label} is not a //package:file label in the main repository"
        ));
    }
    match workspace_label_parts(label) {
        None => Err(format!("{label} is not //package:file")),
        Some(Err(())) => Err(format!("{label} leaves the workspace")),
        Some(Ok(("", file))) => Ok(file.to_owned()),
        Some(Ok((package, file))) => Ok(format!("{package}/{file}")),
    }
}

/// Splits an include label into package and file the way Bazel accepts it.
fn include_parts(label: &str) -> Result<(&str, &str), String> {
    if label.starts_with('@') {
        return Err(format!(
            "include {label} is not a //package:file label; Bazel accepts only // labels in include"
        ));
    }
    let (package, file) = match workspace_label_parts(label) {
        None => return Err(format!("include {label} is not //package:file")),
        Some(Err(())) => return Err(format!("include {label} leaves the workspace")),
        Some(Ok(parts)) => parts,
    };
    let name = file.rsplit('/').next().unwrap_or(file);
    if !name.ends_with(".MODULE.bazel") || name.starts_with('.') {
        return Err(format!("include {label} must name a .MODULE.bazel file"));
    }
    Ok((package, file))
}

/// Resolves an include label to a workspace-relative path under module_dir.
pub fn include_rel(module_dir: &str, label: &str) -> Result<String, String> {
    let (package, file) = include_parts(label)?;
    let mut rel = module_dir.to_owned();
    for part in [package, file] {
        if !part.is_empty() {
            if !rel.is_empty() {
                rel.push('/');
            }
            rel.push_str(part);
        }
    }
    Ok(rel)
}

/// Parses one module file, then recurses into its includes.
fn collect_module(
    text: &str,
    label: Option<&str>,
    read_include: &dyn Fn(&str) -> Result<String, String>,
    seen: &mut BTreeSet<String>,
    files: &mut Vec<(Option<String>, ModuleFile)>,
) -> Result<(), String> {
    let module = ModuleFile::parse(text).map_err(|error| match label {
        Some(label) => format!("{label}: {error}"),
        None => error.to_string(),
    })?;
    let mut labels = Vec::new();
    for call in module.calls_named("include") {
        match call.positional.first().or_else(|| call.arg("label")) {
            Some(Value::Str(label)) => {
                include_parts(&label.value)
                    .map_err(|error| format!("{error} at MODULE.bazel line {}", call.line))?;
                labels.push(label.value.clone());
            }
            _ => {
                return Err(format!(
                    "include at MODULE.bazel line {} needs a string literal label",
                    call.line
                ));
            }
        }
    }
    files.push((label.map(str::to_owned), module));
    for label in labels {
        if !seen.insert(label.clone()) {
            return Err(format!("MODULE.bazel includes {label} more than once"));
        }
        let included = read_include(&label)?;
        collect_module(&included, Some(&label), read_include, seen, files)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

const RULES_DX: &str = "rules_dx";

/// The name the workspace MODULE.bazel (and the files it includes) gives rules_dx,
/// or `rules_dx` when it declares none.
pub fn rules_dx_repo(workspace: &std::path::Path) -> String {
    let read_include = |label: &str| {
        label_path(label)
            .and_then(|rel| std::fs::read_to_string(workspace.join(&rel)).map_err(|_| rel))
            .map_err(|_| format!("workspace input {label}"))
    };
    std::fs::read_to_string(workspace.join("MODULE.bazel")).map_or_else(
        |_| RULES_DX.to_owned(),
        |text| rules_dx_repo_with(&text, &read_include),
    )
}

/// The name a MODULE.bazel source gives rules_dx, ignoring any includes.
pub fn rules_dx_repo_of(module_text: &str) -> String {
    rules_dx_repo_with(module_text, &|_| Err("no includes".to_owned()))
}

/// The name rules_dx gets across a MODULE.bazel and its includes, in include order.
/// When the includes cannot be loaded, falls back to the root file alone.
fn rules_dx_repo_with(
    module_text: &str,
    read_include: &dyn Fn(&str) -> Result<String, String>,
) -> String {
    let repo = match load_with_includes(module_text, read_include) {
        Ok(files) => files.iter().find_map(repo_in_file).map(str::to_owned),
        Err(_) => ModuleFile::parse(module_text)
            .ok()
            .and_then(|file| repo_in_file(&file).map(str::to_owned)),
    };
    match repo {
        Some(repo) if valid_repo_name(&repo) => repo,
        _ => RULES_DX.to_owned(),
    }
}

/// The repo name one module file gives rules_dx, if the file declares rules_dx.
fn repo_in_file(file: &ModuleFile) -> Option<&str> {
    match file
        .calls_named("module")
        .find(|call| call.str("name") == Some(RULES_DX))
    {
        Some(call) => Some(call.str("repo_name").unwrap_or(RULES_DX)),
        None => file.repo_name(RULES_DX),
    }
}

fn valid_repo_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}
