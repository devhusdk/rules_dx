use starlark_syntax::syntax::ast::{AstExpr, AstLiteral, AstStmt, ExprP};
use starlark_syntax::syntax::{AstModule, Dialect};

const STARLARK: [&str; 3] = ["BUILD.bazel", ".BUILD", ".bzl"];

pub fn is_starlark(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "BUILD" || STARLARK.iter().any(|end| name.ends_with(end))
}

pub fn is_bazel_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    ["BUILD", "BUILD.bazel", "MODULE.bazel"].contains(&name) || name.ends_with(".bzl")
}

pub fn is_any_starlark(path: &str) -> bool {
    is_starlark(path) || is_bazel_file(path)
}

pub fn parse(path: &str, source: &str) -> Result<AstModule, String> {
    AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .map_err(|error| format!("{path}: does not parse: {}", error.without_diagnostic()))
}

fn collect<'a>(expr: &'a AstExpr, out: &mut Vec<&'a AstExpr>) {
    out.push(expr);
    expr.visit_expr(|child| collect(child, out));
}

pub fn exprs(stmt: &AstStmt) -> Vec<&AstExpr> {
    let mut out = Vec::new();
    stmt.visit_expr(|expr| collect(expr, &mut out));
    out
}

pub fn callee_name(callee: &AstExpr) -> &str {
    match &callee.node {
        ExprP::Identifier(ident) => &ident.node.ident,
        ExprP::Dot(_, name) => &name.node,
        ExprP::Tuple(_)
        | ExprP::Call(..)
        | ExprP::Index(_)
        | ExprP::Index2(_)
        | ExprP::Slice(..)
        | ExprP::Lambda(_)
        | ExprP::Literal(_)
        | ExprP::Not(_)
        | ExprP::Minus(_)
        | ExprP::Plus(_)
        | ExprP::BitNot(_)
        | ExprP::Op(..)
        | ExprP::If(_)
        | ExprP::List(_)
        | ExprP::Dict(_)
        | ExprP::ListComprehension(..)
        | ExprP::DictComprehension(..)
        | ExprP::FString(_) => "",
    }
}

pub fn string(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(text)) => Some(&text.node),
        ExprP::Tuple(_)
        | ExprP::Dot(..)
        | ExprP::Call(..)
        | ExprP::Index(_)
        | ExprP::Index2(_)
        | ExprP::Slice(..)
        | ExprP::Identifier(_)
        | ExprP::Lambda(_)
        | ExprP::Literal(_)
        | ExprP::Not(_)
        | ExprP::Minus(_)
        | ExprP::Plus(_)
        | ExprP::BitNot(_)
        | ExprP::Op(..)
        | ExprP::If(_)
        | ExprP::List(_)
        | ExprP::Dict(_)
        | ExprP::ListComprehension(..)
        | ExprP::DictComprehension(..)
        | ExprP::FString(_) => None,
    }
}

pub fn is_true(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Identifier(ident) if ident.node.ident == "True")
}
