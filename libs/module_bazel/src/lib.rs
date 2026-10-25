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
        let repo = self.repo_name(MAVEN_MODULE).unwrap_or(MAVEN_MODULE);
        let label = format!("@{repo}//:extensions.bzl");
        let proxies: Vec<&str> = self
            .calls_named("use_extension")
            .filter(|call| {
                let file = extension_arg(call, 0, "extension_bzl_file");
                let extension = extension_arg(call, 1, "extension_name");
                file == Some(label.as_str()) && extension == Some("maven")
            })
            .filter_map(|call| call.binding.as_deref())
            .collect();
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

#[cfg(test)]
mod tests;
