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
    #[error("{value:?} cannot be written into a Starlark string literal")]
    UnsafeString { value: String },
    #[error("{name:?} is already declared in MODULE.bazel")]
    AlreadyDeclared { name: String },
    #[error("cannot insert into MODULE.bazel: {reason}")]
    NoInsertPoint { reason: String },
}

/// A parsed MODULE.bazel with its top-level calls in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFile {
    source: String,
    calls: Vec<Call>,
}

const MAVEN_MODULE: &str = "rules_jvm_external";

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
        let ast =
            AstModule::parse("MODULE.bazel", source.to_owned(), &dialect()).map_err(|error| {
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

    /// The repository name module `name` is visible as, if it is declared.
    pub fn repo_name(&self, name: &str) -> Option<&str> {
        self.calls_named("bazel_dep")
            .find(|call| call.str("name") == Some(name))
            .and_then(|call| call.str("repo_name").or_else(|| call.str("name")))
    }

    /// Every literal coordinate in the `artifacts` of rules_jvm_external `maven.install` tags.
    pub fn maven_artifacts(&self) -> Result<Vec<&Str>, ModuleError> {
        let proxies = self.maven_proxies();
        let mut artifacts = Vec::new();
        for call in &self.calls {
            let Some((proxy, "install")) = call.name.rsplit_once('.') else {
                continue;
            };
            if !proxies.contains(&proxy) {
                continue;
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
                })
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
                    })
                }
                None => installs.push(call),
                Some(Value::Str(literal)) if literal.value == "maven" => installs.push(call),
                Some(Value::Str(_)) => {}
                Some(_) => {
                    return Err(ModuleError::NoInsertPoint {
                        reason: format!("maven.install at line {} has a computed name", call.line),
                    })
                }
            }
        }
        let call = match installs.as_slice() {
            [call] => *call,
            [] => {
                return Err(ModuleError::NoInsertPoint {
                    reason: "no maven.install tag for the default @maven repository".to_owned(),
                })
            }
            many => {
                return Err(ModuleError::NoInsertPoint {
                    reason: format!(
                        "{} maven.install tags feed the default @maven repository",
                        many.len()
                    ),
                })
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

    fn maven_proxies(&self) -> Vec<&str> {
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
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    collect_module(root_text, None, read_include, &mut seen, &mut files)?;
    Ok(files)
}

/// Splits an include label into package and file the way Bazel accepts it.
fn include_parts(label: &str) -> Result<(&str, &str), String> {
    if label.starts_with('@') {
        return Err(format!(
            "include {label} is not a //package:file label; Bazel accepts only // labels in include"
        ));
    }
    let (package, file) = label
        .strip_prefix("//")
        .and_then(|rest| rest.split_once(':'))
        .ok_or_else(|| format!("include {label} is not //package:file"))?;
    let bad =
        |part: &str| part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']);
    let package_bad = !package.is_empty() && package.split('/').any(bad);
    if package_bad || file.split('/').any(bad) {
        return Err(format!("include {label} leaves the workspace"));
    }
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
    files: &mut Vec<ModuleFile>,
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
                ))
            }
        }
    }
    files.push(module);
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
