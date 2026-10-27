//! Reads `target_compatible_with` and `tags` of one test from its BUILD file.
//!
//! The reader fails closed: any expression it cannot evaluate exactly is an error.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Str(String),
    Ident(String),
    Op(String),
    Other,
}

#[derive(Clone, Debug)]
struct Token {
    tok: Tok,
    col: usize,
    depth: usize,
}

fn lex(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut col = 0;
    let mut depth = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            i += 1;
            col = 0;
            continue;
        }
        if c == ' ' || c == '\t' || c == '\r' {
            i += 1;
            col += 1;
            continue;
        }
        if c == '\\' && chars.get(i + 1) == Some(&'\n') {
            i += 2;
            col = 0;
            continue;
        }
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        let start_col = col;
        let raw = (c == 'r' || c == 'b') && matches!(chars.get(i + 1), Some('"') | Some('\''));
        if c == '"' || c == '\'' || raw {
            if raw {
                i += 1;
            }
            let quote = chars[i];
            let triple = chars.get(i + 1) == Some(&quote) && chars.get(i + 2) == Some(&quote);
            i += if triple { 3 } else { 1 };
            let mut value = String::new();
            loop {
                let Some(&ch) = chars.get(i) else {
                    return Err("unterminated string".to_owned());
                };
                if triple {
                    if ch == quote
                        && chars.get(i + 1) == Some(&quote)
                        && chars.get(i + 2) == Some(&quote)
                    {
                        i += 3;
                        break;
                    }
                } else if ch == quote {
                    i += 1;
                    break;
                } else if ch == '\n' {
                    return Err("unterminated string".to_owned());
                }
                if ch == '\\' && !raw {
                    let Some(&next) = chars.get(i + 1) else {
                        return Err("unterminated string".to_owned());
                    };
                    value.push(match next {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                    i += 2;
                    continue;
                }
                value.push(ch);
                i += 1;
            }
            out.push(Token {
                tok: Tok::Str(value),
                col: start_col,
                depth,
            });
            col = start_col + 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let mut ident = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                ident.push(chars[i]);
                i += 1;
                col += 1;
            }
            out.push(Token {
                tok: Tok::Ident(ident),
                col: start_col,
                depth,
            });
            continue;
        }
        if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
                col += 1;
            }
            out.push(Token {
                tok: Tok::Other,
                col: start_col,
                depth,
            });
            continue;
        }
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let op = if ["==", "!=", "<=", ">=", "+=", "|=", "-=", "**", "//"].contains(&two.as_str()) {
            two
        } else {
            c.to_string()
        };
        i += op.chars().count();
        col += op.chars().count();
        if op == ")" || op == "]" || op == "}" {
            depth = depth.saturating_sub(1);
        }
        out.push(Token {
            tok: Tok::Op(op.clone()),
            col: start_col,
            depth,
        });
        if op == "(" || op == "[" || op == "{" {
            depth += 1;
        }
    }
    Ok(out)
}

fn is_op(token: Option<&Token>, op: &str) -> bool {
    matches!(token, Some(Token { tok: Tok::Op(found), .. }) if found == op)
}

/// One top-level rule call: its kind and the raw tokens of each keyword argument.
#[derive(Clone, Debug)]
struct Call {
    kind: String,
    args: BTreeMap<String, Vec<Token>>,
}

/// One parsed Starlark file: top-level assignments, loads and rule calls.
#[derive(Clone, Debug, Default)]
pub struct File {
    path: String,
    assigns: BTreeMap<String, Vec<Token>>,
    loads: BTreeMap<String, (String, String)>,
    calls: Vec<Call>,
}

/// Returns the tokens of one expression that starts at `start`, ending at a depth-0 `,` or close.
fn expr_end(tokens: &[Token], start: usize, base: usize) -> usize {
    let mut i = start;
    while i < tokens.len() {
        let token = &tokens[i];
        if token.depth == base && is_op(Some(token), ",") {
            break;
        }
        if token.depth < base {
            break;
        }
        i += 1;
    }
    i
}

/// Splits the depth-`base + 1` arguments of a call that opens at `open`.
fn call_args(tokens: &[Token], open: usize) -> (Vec<Vec<Token>>, usize) {
    let inner = tokens[open].depth + 1;
    let mut args = Vec::new();
    let mut i = open + 1;
    while i < tokens.len() && !(tokens[i].depth == inner - 1 && is_op(Some(&tokens[i]), ")")) {
        let end = expr_end(tokens, i, inner);
        if end > i {
            args.push(tokens[i..end].to_vec());
        }
        i = end;
        if is_op(tokens.get(i), ",") {
            i += 1;
        }
    }
    (args, i + 1)
}

/// Parses the top-level statements of a BUILD or `.bzl` file.
pub fn parse(path: &str, text: &str) -> Result<File, String> {
    let tokens = lex(text).map_err(|error| format!("{path}: {error}"))?;
    let mut file = File {
        path: path.to_owned(),
        ..File::default()
    };
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let top = token.col == 0 && token.depth == 0;
        let Tok::Ident(ident) = &token.tok else {
            i += 1;
            continue;
        };
        if !top {
            i += 1;
            continue;
        }
        if is_op(tokens.get(i + 1), "=") {
            let mut stop = i + 2;
            while stop < tokens.len()
                && !(tokens[stop].col == 0
                    && tokens[stop].depth == 0
                    && matches!(tokens[stop].tok, Tok::Ident(_)))
            {
                stop += 1;
            }
            file.assigns
                .insert(ident.clone(), tokens[i + 2..stop].to_vec());
            i = stop;
            continue;
        }
        if is_op(tokens.get(i + 1), "(") {
            let (args, next) = call_args(&tokens, i + 1);
            if ident == "load" {
                let mut source = None;
                for arg in &args {
                    match arg.as_slice() {
                        [Token {
                            tok: Tok::Str(text),
                            ..
                        }] if source.is_none() => {
                            source = Some(text.clone());
                        }
                        [Token {
                            tok: Tok::Str(text),
                            ..
                        }] => {
                            file.loads.insert(
                                text.clone(),
                                (source.clone().unwrap_or_default(), text.clone()),
                            );
                        }
                        [Token {
                            tok: Tok::Ident(local),
                            ..
                        }, eq, Token {
                            tok: Tok::Str(text),
                            ..
                        }] if is_op(Some(eq), "=") => {
                            file.loads.insert(
                                local.clone(),
                                (source.clone().unwrap_or_default(), text.clone()),
                            );
                        }
                        _ => return Err(format!("{path}: cannot read a load() statement")),
                    }
                }
            } else {
                let mut call = Call {
                    kind: ident.clone(),
                    args: BTreeMap::new(),
                };
                for arg in args {
                    if let [Token {
                        tok: Tok::Ident(key),
                        ..
                    }, eq, rest @ ..] = arg.as_slice()
                    {
                        if is_op(Some(eq), "=") {
                            call.args.insert(key.clone(), rest.to_vec());
                        }
                    }
                }
                file.calls.push(call);
            }
            i = next;
            continue;
        }
        i += 1;
    }
    Ok(file)
}

/// A list value: plain parts and `select()` parts, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Fixed(Vec<String>),
    Select(Vec<(String, Vec<String>)>),
    AnyOf(Vec<Vec<String>>),
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Str(String),
    List(Vec<Part>),
}

/// Resolves `load()` labels to parsed files, one hop only.
pub trait Loader {
    fn load(&self, label: &str) -> Result<File, String>;

    /// Returns the alternatives Bazel computed for a constant the reader cannot evaluate.
    fn computed(&self, label: &str, symbol: &str) -> Option<Vec<Vec<String>>>;
}

struct Eval<'a> {
    file: &'a File,
    loader: Option<&'a dyn Loader>,
    what: &'a str,
}

impl Eval<'_> {
    fn err(&self, message: &str) -> String {
        format!("{} {}: {message}", self.file.path, self.what)
    }

    fn sum(&self, tokens: &[Token]) -> Result<Value, String> {
        let base = tokens.first().map_or(0, |token| token.depth);
        let mut terms = Vec::new();
        let mut start = 0;
        for (index, token) in tokens.iter().enumerate() {
            if token.depth == base && is_op(Some(token), "+") {
                terms.push(&tokens[start..index]);
                start = index + 1;
            }
        }
        terms.push(&tokens[start..]);
        let mut value: Option<Value> = None;
        for term in terms {
            let next = self.atom(term)?;
            value = Some(match (value, next) {
                (None, next) => next,
                (Some(Value::List(mut left)), Value::List(right)) => {
                    left.extend(right);
                    Value::List(left)
                }
                (Some(Value::Str(left)), Value::Str(right)) => Value::Str(left + &right),
                _ => return Err(self.err("`+` mixes a string and a list")),
            });
        }
        value.ok_or_else(|| self.err("empty expression"))
    }

    fn atom(&self, tokens: &[Token]) -> Result<Value, String> {
        match tokens {
            [] => Err(self.err("empty expression")),
            [Token {
                tok: Tok::Str(_), ..
            }, ..]
                if tokens.iter().all(|token| matches!(token.tok, Tok::Str(_))) =>
            {
                let mut text = String::new();
                for token in tokens {
                    if let Tok::Str(part) = &token.tok {
                        text.push_str(part);
                    }
                }
                Ok(Value::Str(text))
            }
            [Token {
                tok: Tok::Op(open), ..
            }, inner @ .., Token {
                tok: Tok::Op(close),
                ..
            }] if open == "[" && close == "]" && wrapped(tokens) => {
                let mut items = Vec::new();
                for item in split_commas(inner) {
                    match self.sum(item)? {
                        Value::Str(text) => items.push(text),
                        Value::List(_) => return Err(self.err("a list item is not a string")),
                    }
                }
                Ok(Value::List(vec![Part::Fixed(items)]))
            }
            [Token {
                tok: Tok::Op(open), ..
            }, inner @ .., Token {
                tok: Tok::Op(close),
                ..
            }] if open == "(" && close == ")" && wrapped(tokens) => self.sum(inner),
            [Token {
                tok: Tok::Ident(name),
                ..
            }, Token {
                tok: Tok::Op(open), ..
            }, inner @ .., Token {
                tok: Tok::Op(close),
                ..
            }] if name == "select" && open == "(" && close == ")" && wrapped(&tokens[1..]) => {
                self.select(inner)
            }
            [Token {
                tok: Tok::Ident(name),
                ..
            }, Token {
                tok: Tok::Op(open), ..
            }, ..]
                if open == "(" =>
            {
                Err(self.err(&format!("cannot evaluate the call {name}()")))
            }
            [Token {
                tok: Tok::Ident(name),
                ..
            }] => self.resolve(name),
            _ => Err(self.err("cannot evaluate the expression")),
        }
    }

    fn select(&self, tokens: &[Token]) -> Result<Value, String> {
        let [Token {
            tok: Tok::Op(open), ..
        }, inner @ .., Token {
            tok: Tok::Op(close),
            ..
        }] = tokens
        else {
            return Err(self.err("select() takes one dict"));
        };
        if open != "{" || close != "}" || !wrapped(tokens) {
            return Err(self.err("select() takes one dict"));
        }
        let mut branches = Vec::new();
        for entry in split_commas(inner) {
            let base = entry.first().map_or(0, |token| token.depth);
            let Some(colon) = entry
                .iter()
                .position(|token| token.depth == base && is_op(Some(token), ":"))
            else {
                return Err(self.err("a select() entry has no `:`"));
            };
            let Value::Str(key) = self.sum(&entry[..colon])? else {
                return Err(self.err("a select() key is not a string"));
            };
            let Value::List(parts) = self.sum(&entry[colon + 1..])? else {
                return Err(self.err("a select() branch is not a list"));
            };
            let mut items = Vec::new();
            for part in parts {
                match part {
                    Part::Fixed(values) => items.extend(values),
                    Part::Select(_) | Part::AnyOf(_) => return Err(self.err("nested select()")),
                }
            }
            branches.push((key, items));
        }
        Ok(Value::List(vec![Part::Select(branches)]))
    }

    fn resolve(&self, name: &str) -> Result<Value, String> {
        if let Some(tokens) = self.file.assigns.get(name) {
            return self.sum(tokens);
        }
        if let Some((label, symbol)) = self.file.loads.get(name) {
            if let Some(any_of) = self
                .loader
                .and_then(|loader| loader.computed(label, symbol))
            {
                return Ok(Value::List(vec![Part::AnyOf(any_of)]));
            }
            let Some(loader) = self.loader else {
                return Err(self.err(&format!(
                    "{name} comes from a second load() hop, which is not followed"
                )));
            };
            let loaded = loader.load(label)?;
            let next = Eval {
                file: &loaded,
                loader: None,
                what: self.what,
            };
            let Some(tokens) = loaded.assigns.get(symbol) else {
                return Err(self.err(&format!("{label} defines no constant {symbol}")));
            };
            return next.sum(tokens);
        }
        Err(self.err(&format!("unknown name {name}")))
    }
}

/// Reports whether the first and last tokens are one bracket pair around the rest.
fn wrapped(tokens: &[Token]) -> bool {
    let Some(first) = tokens.first() else {
        return false;
    };
    tokens.len() >= 2
        && tokens[1..tokens.len() - 1]
            .iter()
            .all(|token| token.depth > first.depth)
}

fn split_commas(tokens: &[Token]) -> Vec<&[Token]> {
    let Some(first) = tokens.first() else {
        return Vec::new();
    };
    let base = first.depth;
    let mut out = Vec::new();
    let mut start = 0;
    for (index, token) in tokens.iter().enumerate() {
        if token.depth == base && is_op(Some(token), ",") {
            out.push(&tokens[start..index]);
            start = index + 1;
        }
    }
    if start < tokens.len() {
        out.push(&tokens[start..]);
    }
    out
}

/// The facts read for one test target.
#[derive(Clone, Debug, PartialEq)]
pub struct TestFacts {
    pub compatible_with: Vec<Part>,
    pub tags: Vec<String>,
}

/// How a rule kind's macro changes `target_compatible_with` before the rule sees it.
#[derive(Clone, Copy, Debug)]
enum Constraint {
    /// The macro forwards the attribute unchanged.
    AsWritten,
    /// The macro sets the constant `label%symbol` when the call leaves the attribute out.
    DefaultIfUnset(&'static str, &'static str),
    /// The macro always appends the constant `label%symbol` to the call's value.
    Appended(&'static str, &'static str),
}

/// Rule kinds the reader knows, with what each macro does to `target_compatible_with`.
///
/// A kind whose macro computes its own constraints must say so here, or the matrix
/// would claim platforms the test never runs on. rust_test's libstdc++ select only
/// guards the upstream target outside its runtime transition, and the cc, go, haskell,
/// kotlin, typescript and elixir wrappers forward the attribute as written.
const KNOWN_KINDS: &[(&str, Constraint)] = &[
    ("cc_test", Constraint::AsWritten),
    ("csharp_test", Constraint::AsWritten),
    (
        "cuda_gpu_test",
        Constraint::Appended("//cuda/rules:defs.bzl", "CUDA_TARGET"),
    ),
    ("d_test", Constraint::AsWritten),
    ("eunit", Constraint::AsWritten),
    ("ex_unit_test", Constraint::AsWritten),
    (
        "fortran_test",
        Constraint::DefaultIfUnset("//fortran/rules:defs.bzl", "FORTRAN_COMPATIBLE_WITH"),
    ),
    ("fsharp_test", Constraint::AsWritten),
    ("go_test", Constraint::AsWritten),
    ("haskell_test", Constraint::AsWritten),
    ("java_test", Constraint::AsWritten),
    ("javascript_test", Constraint::AsWritten),
    ("julia_test", Constraint::AsWritten),
    ("kotlin_test", Constraint::AsWritten),
    ("lua_test", Constraint::AsWritten),
    (
        "ocaml_test",
        Constraint::DefaultIfUnset("//ocaml/rules:defs.bzl", "OCAML_COMPATIBLE_WITH"),
    ),
    (
        "perl_test",
        Constraint::Appended("//perl/rules:defs.bzl", "PERL_COMPATIBLE_WITH"),
    ),
    (
        "php_test",
        Constraint::Appended("//php/rules:defs.bzl", "PHP_COMPATIBLE_WITH"),
    ),
    ("pwsh_test", Constraint::AsWritten),
    ("python_test", Constraint::AsWritten),
    ("ruby_test", Constraint::AsWritten),
    ("rust_test", Constraint::AsWritten),
    ("scala_test", Constraint::AsWritten),
    ("sh_test", Constraint::AsWritten),
    ("typescript_test", Constraint::AsWritten),
    ("zig_test", Constraint::AsWritten),
    (
        "nested_bazel_test",
        Constraint::DefaultIfUnset(
            "//tools/testing:nested_bazel.bzl",
            "NESTED_BAZEL_COMPATIBLE_WITH",
        ),
    ),
];

/// Evaluates a macro's constant: Bazel's computed value first, else the parsed file.
fn macro_constant(
    eval: &Eval,
    loader: &dyn Loader,
    label: &str,
    symbol: &str,
) -> Result<Vec<Part>, String> {
    if let Some(any_of) = loader.computed(label, symbol) {
        return Ok(vec![Part::AnyOf(any_of)]);
    }
    let loaded = loader.load(label)?;
    let inner = Eval {
        file: &loaded,
        loader: None,
        what: eval.what,
    };
    let Some(tokens) = loaded.assigns.get(symbol) else {
        return Err(eval.err(&format!("{label} defines no constant {symbol}")));
    };
    match inner.sum(tokens)? {
        Value::List(parts) => Ok(parts),
        Value::Str(_) => Err(eval.err("default constraints are a string")),
    }
}

/// Reads the facts of `name` from the parsed BUILD file `file`.
pub fn test_facts(file: &File, name: &str, loader: &dyn Loader) -> Result<TestFacts, String> {
    let what = format!(":{name}");
    let eval = Eval {
        file,
        loader: Some(loader),
        what: &what,
    };
    let mut found = None;
    for call in &file.calls {
        let Some(tokens) = call.args.get("name") else {
            continue;
        };
        if eval.sum(tokens).ok() == Some(Value::Str(name.to_owned())) {
            if found.is_some() {
                return Err(eval.err("declared twice"));
            }
            found = Some(call);
        }
    }
    let Some(call) = found else {
        return Err(eval.err("is not declared by a direct rule call"));
    };
    let Some((_, constraint)) = KNOWN_KINDS.iter().find(|(kind, _)| *kind == call.kind) else {
        return Err(eval.err(&format!(
            "rule kind {} is unknown to the support matrix reader",
            call.kind
        )));
    };
    let written = match call.args.get("target_compatible_with") {
        Some(tokens) => match eval.sum(tokens)? {
            Value::List(parts) => Some(parts),
            Value::Str(_) => return Err(eval.err("target_compatible_with is a string")),
        },
        None => None,
    };
    let compatible_with = match (written, *constraint) {
        (Some(parts), Constraint::AsWritten | Constraint::DefaultIfUnset(..)) => parts,
        (None, Constraint::AsWritten) => Vec::new(),
        (None, Constraint::DefaultIfUnset(label, symbol)) => {
            macro_constant(&eval, loader, label, symbol)?
        }
        (written, Constraint::Appended(label, symbol)) => {
            let mut parts = written.unwrap_or_default();
            parts.extend(macro_constant(&eval, loader, label, symbol)?);
            parts
        }
    };
    let tags = match call.args.get("tags") {
        None => Vec::new(),
        Some(tokens) => match eval.sum(tokens)? {
            Value::List(parts) => {
                let mut tags = Vec::new();
                for part in parts {
                    match part {
                        Part::Fixed(values) => tags.extend(values),
                        Part::Select(_) | Part::AnyOf(_) => {
                            return Err(eval.err("tags use select()"))
                        }
                    }
                }
                tags
            }
            Value::Str(_) => return Err(eval.err("tags is a string")),
        },
    };
    Ok(TestFacts {
        compatible_with,
        tags,
    })
}

/// One CI platform as the reader sees it.
#[derive(Clone, Debug)]
pub struct Platform {
    pub id: String,
    pub os: String,
    pub cpu: String,
}

fn cpu_alias(cpu: &str) -> &str {
    if cpu == "aarch64" {
        "arm64"
    } else {
        cpu
    }
}

fn select_key_matches(key: &str, platform: &Platform) -> Result<bool, String> {
    if let Some(os) = key.strip_prefix("@platforms//os:") {
        return Ok(os == platform.os);
    }
    if let Some(cpu) = key.strip_prefix("@platforms//cpu:") {
        return Ok(cpu_alias(cpu) == cpu_alias(&platform.cpu));
    }
    Err(format!("select() key {key} is not an @platforms os or cpu"))
}

fn constraint_holds(constraint: &str, platform: &Platform) -> bool {
    if let Some(os) = constraint.strip_prefix("@platforms//os:") {
        return os == platform.os;
    }
    if let Some(cpu) = constraint.strip_prefix("@platforms//cpu:") {
        return cpu_alias(cpu) == cpu_alias(&platform.cpu);
    }
    false
}

/// Reports whether a test with `parts` constraints is compatible with `platform`.
pub fn compatible(parts: &[Part], platform: &Platform) -> Result<bool, String> {
    let mut constraints = Vec::new();
    for part in parts {
        match part {
            Part::Fixed(values) => constraints.extend(values.iter().cloned()),
            Part::AnyOf(alternatives) => {
                let holds = alternatives.iter().any(|alternative| {
                    alternative
                        .iter()
                        .all(|constraint| constraint_holds(constraint, platform))
                });
                if !holds {
                    constraints.push("@platforms//:incompatible".to_owned());
                }
            }
            Part::Select(branches) => {
                let mut chosen = None;
                let mut default = None;
                for (key, values) in branches {
                    if key == "//conditions:default" {
                        default = Some(values);
                        continue;
                    }
                    if select_key_matches(key, platform)? {
                        if chosen.is_some() {
                            return Err(format!("select() has two branches for {}", platform.id));
                        }
                        chosen = Some(values);
                    }
                }
                let Some(values) = chosen.or(default) else {
                    return Err(format!("select() has no branch for {}", platform.id));
                };
                constraints.extend(values.iter().cloned());
            }
        }
    }
    Ok(constraints
        .iter()
        .all(|constraint| constraint_holds(constraint, platform)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Files(BTreeMap<String, String>);

    impl Loader for Files {
        fn load(&self, label: &str) -> Result<File, String> {
            let text = self
                .0
                .get(label)
                .ok_or_else(|| format!("no fixture for {label}"))?;
            parse(label, text)
        }

        fn computed(&self, label: &str, symbol: &str) -> Option<Vec<Vec<String>>> {
            if label == "//perl/rules:defs.bzl" && symbol == "PERL_COMPATIBLE_WITH" {
                return Some(vec![
                    vec![
                        "@platforms//os:linux".to_owned(),
                        "@platforms//cpu:x86_64".to_owned(),
                    ],
                    vec![
                        "@platforms//os:windows".to_owned(),
                        "@platforms//cpu:x86_64".to_owned(),
                    ],
                ]);
            }
            (label == "//d:defs.bzl" && symbol == "D").then(|| {
                vec![
                    vec!["@platforms//os:linux".to_owned()],
                    vec![
                        "@platforms//os:macos".to_owned(),
                        "@platforms//cpu:aarch64".to_owned(),
                    ],
                ]
            })
        }
    }

    fn loader() -> Files {
        let linux_x86_64 = "[\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"]\n".to_owned();
        Files(BTreeMap::from([
            (
                "//fortran/rules:defs.bzl".to_owned(),
                format!("FORTRAN_COMPATIBLE_WITH = {linux_x86_64}"),
            ),
            (
                "//php/rules:defs.bzl".to_owned(),
                format!("PHP_COMPATIBLE_WITH = {linux_x86_64}"),
            ),
            (
                "//tools/testing:nested_bazel.bzl".to_owned(),
                "\"\"\"Doc.\"\"\"\n\nTAGS = [\n    \"local\",\n]\n\nNET_TAGS = TAGS + [\"requires-network\"]\n\nNESTED_BAZEL_COMPATIBLE_WITH = []\n\ndef f(x):\n    y = 1\n    return x\n".to_owned(),
            ),
        ]))
    }

    fn platform(id: &str) -> Platform {
        let (os, cpu) = id.split_once('_').expect("os_cpu");
        Platform {
            id: id.to_owned(),
            os: os.to_owned(),
            cpu: cpu.to_owned(),
        }
    }

    fn on(facts: &TestFacts) -> Vec<&'static str> {
        [
            "linux_x86_64",
            "linux_arm64",
            "macos_arm64",
            "windows_x86_64",
            "windows_arm64",
        ]
        .into_iter()
        .filter(|id| compatible(&facts.compatible_with, &platform(id)).expect("compat"))
        .collect()
    }

    fn facts(build: &str, name: &str) -> Result<TestFacts, String> {
        let file = parse("pkg/BUILD.bazel", build)?;
        test_facts(&file, name, &loader())
    }

    #[test]
    fn reads_a_literal_list() {
        let found = facts(
            "load(\"//x:y.bzl\", \"z\")\n\nrust_test(\n    name = \"t\",\n    data = glob([\"*.rs\"]),\n    target_compatible_with = [\n        \"@platforms//os:linux\",\n        \"@platforms//cpu:x86_64\",\n    ],\n)\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64"]);
        assert!(found.tags.is_empty());
    }

    #[test]
    fn reads_calls_after_a_constant() {
        let found = facts(
            "_L = [\"@platforms//os:linux\"]\n\nrust_test(name = \"t\", target_compatible_with = _L)\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64", "linux_arm64"]);
    }

    #[test]
    fn missing_constraints_mean_every_platform() {
        let found = facts("rust_test(name = \"t\")\n", "t").expect("facts");
        assert_eq!(on(&found).len(), 5);
    }

    #[test]
    fn follows_one_load_hop_and_concatenation() {
        let found = facts(
            "load(\"//tools/testing:nested_bazel.bzl\", \"NET_TAGS\", C = \"NESTED_BAZEL_COMPATIBLE_WITH\")\n\nrust_test(\n    name = \"t\",\n    tags = NET_TAGS + [\"manual\"],\n    target_compatible_with = C,\n)\n",
            "t",
        )
        .expect("facts");
        assert_eq!(found.tags, ["local", "requires-network", "manual"]);
        assert_eq!(on(&found).len(), 5);
    }

    #[test]
    fn macro_default_comes_from_its_constant() {
        let found = facts("nested_bazel_test(name = \"t\")\n", "t").expect("facts");
        assert_eq!(on(&found).len(), 5);
    }

    #[test]
    fn macro_default_applies_only_when_unset() {
        let found = facts("fortran_test(name = \"t\")\n", "t").expect("facts");
        assert_eq!(on(&found), ["linux_x86_64"]);
        let found = facts(
            "fortran_test(name = \"t\", target_compatible_with = [\"@platforms//os:linux\"])\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64", "linux_arm64"]);
    }

    #[test]
    fn macro_appends_its_constant_to_the_call() {
        let found = facts("php_test(name = \"t\")\n", "t").expect("facts");
        assert_eq!(on(&found), ["linux_x86_64"]);
        let found = facts(
            "php_test(name = \"t\", target_compatible_with = [\"@platforms//os:linux\"])\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64"]);
        let found = facts(
            "perl_test(name = \"t\", target_compatible_with = [\"@platforms//cpu:x86_64\"])\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64", "windows_x86_64"]);
    }

    #[test]
    fn evaluates_select_on_os_and_cpu() {
        let found = facts(
            "_LINUX = select({\n    \"@platforms//cpu:arm64\": [\"@platforms//os:linux\"],\n    \"@platforms//cpu:x86_64\": [\"@platforms//os:linux\"],\n    \"//conditions:default\": [\"@platforms//:incompatible\"],\n})\n\nrust_test(\n    name = \"t\",\n    target_compatible_with = _LINUX,\n)\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64", "linux_arm64"]);
    }

    #[test]
    fn reads_a_computed_constant() {
        let found = facts(
            "load(\"//d:defs.bzl\", \"D\")\n\nrust_test(name = \"t\", target_compatible_with = D)\n",
            "t",
        )
        .expect("facts");
        assert_eq!(on(&found), ["linux_x86_64", "linux_arm64", "macos_arm64"]);
    }

    #[test]
    fn unknown_constraints_hold_nowhere() {
        let found = facts(
            "rust_test(name = \"t\", target_compatible_with = [\"//cuda:gpu\"])\n",
            "t",
        )
        .expect("facts");
        assert!(on(&found).is_empty());
    }

    #[test]
    fn fails_closed_on_a_call() {
        let error = facts(
            "rust_test(name = \"t\", target_compatible_with = host_only())\n",
            "t",
        )
        .expect_err("a call is not evaluated");
        assert!(error.contains("pkg/BUILD.bazel :t"), "{error}");
    }

    #[test]
    fn fails_closed_on_an_unknown_kind_or_name() {
        assert!(facts("my_macro(name = \"t\")\n", "t").is_err());
        assert!(facts("rust_test(name = \"t\")\n", "u").is_err());
        assert!(facts("rust_test(name = \"t\", target_compatible_with = X)\n", "t").is_err());
    }

    #[test]
    fn fails_closed_on_a_second_hop() {
        let error = facts(
            "load(\"//tools/testing:nested_bazel.bzl\", \"f\")\n\nrust_test(name = \"t\", tags = f)\n",
            "t",
        )
        .expect_err("no constant f");
        assert!(error.contains("defines no constant f"), "{error}");
    }

    #[test]
    fn fails_closed_on_an_unknown_select_key() {
        let found = facts(
            "rust_test(name = \"t\", target_compatible_with = select({\"@dx_qt//:linux\": [], \"//conditions:default\": []}))\n",
            "t",
        )
        .expect("facts");
        assert!(compatible(&found.compatible_with, &platform("linux_x86_64")).is_err());
    }
}
