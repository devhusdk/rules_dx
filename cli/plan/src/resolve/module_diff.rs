use std::collections::{BTreeMap, BTreeSet};

use dx_module_bazel::{Call, ModuleFile, Value};

use super::extension_scope::{Names, ROOT, Scope, module_base};
use super::lock_inputs::go_repo_name;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModuleDiff {
    pub(crate) deps: BTreeMap<String, String>,
    pub(crate) modules: BTreeSet<String>,
    pub(crate) scopes: BTreeSet<Scope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    path: Option<String>,
    text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Usage {
    entries: Vec<Entry>,
    repos: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Dep {
    version: Option<String>,
    rest: String,
}

#[derive(Debug, Default)]
struct Side {
    deps: BTreeMap<String, Dep>,
    overrides: BTreeMap<String, Vec<String>>,
    usages: BTreeMap<Scope, Usage>,
    rules: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    other: Vec<(String, String)>,
}

enum Binding {
    Extension(Scope),
    Rule(String),
}

const GO_PATH_TAGS: &[&str] = &[
    "module",
    "module_override",
    "archive_override",
    "gazelle_override",
];

const DEV_USAGE: &str = "use_extension(dev_dependency=True,)";

fn go_deps() -> Scope {
    Scope::extension("gazelle+", "go_deps")
}

fn changes(what: &str) -> dx_error::DxError {
    dx_error::DxError::failure(format!("MODULE.bazel changes {what}"))
}

fn render_value(value: &Value, out: &mut String) -> bool {
    match value {
        Value::Str(literal) => out.push_str(&format!("{:?}", literal.value)),
        Value::Ident(name) => out.push_str(name),
        Value::List(items) => {
            out.push('[');
            for item in items {
                if !render_value(item, out) {
                    return false;
                }
                out.push(',');
            }
            out.push(']');
        }
        Value::Dict(entries) => {
            out.push('{');
            for (key, item) in entries {
                if !render_value(key, out) {
                    return false;
                }
                out.push(':');
                if !render_value(item, out) {
                    return false;
                }
                out.push(',');
            }
            out.push('}');
        }
        Value::Computed { .. } => return false,
    }
    true
}

fn render(call: &Call, source: &str, name: &str, skip: usize, keys: &[&str]) -> String {
    let raw = || {
        let text = source.get(call.span.clone()).unwrap_or_default();
        format!(
            "raw {}",
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        )
    };
    if call.unpacked {
        return raw();
    }
    let mut out = format!("{name}(");
    for value in call.positional.iter().skip(skip) {
        if !render_value(value, &mut out) {
            return raw();
        }
        out.push(',');
    }
    let mut named: Vec<&(String, Value)> = call
        .named
        .iter()
        .filter(|(key, _)| !keys.contains(&key.as_str()))
        .collect();
    named.sort_by(|left, right| left.0.cmp(&right.0));
    for (key, value) in named {
        out.push_str(key);
        out.push('=');
        if !render_value(value, &mut out) {
            return raw();
        }
        out.push(',');
    }
    out.push(')');
    out
}

fn strip_comments(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    let mut quote: Option<(char, bool)> = None;
    while let Some(ch) = chars.next() {
        match quote {
            None if ch == '#' => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            None if ch == '"' || ch == '\'' => {
                out.push(ch);
                let triple = text_starts_triple(&mut chars, ch, &mut out);
                quote = Some((ch, triple));
            }
            None => out.push(ch),
            Some(_) if ch == '\\' => {
                out.push(ch);
                out.extend(chars.next());
            }
            Some((open, triple)) if ch == open => {
                out.push(ch);
                if !triple {
                    quote = None;
                } else if chars.peek() == Some(&open) {
                    out.extend(chars.next());
                    if chars.peek() == Some(&open) {
                        out.extend(chars.next());
                        quote = None;
                    }
                }
            }
            Some(_) => out.push(ch),
        }
    }
    out
}

fn text_starts_triple(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    open: char,
    out: &mut String,
) -> bool {
    if chars.peek() != Some(&open) {
        return false;
    }
    out.extend(chars.next());
    if chars.peek() == Some(&open) {
        out.extend(chars.next());
        return true;
    }
    false
}

fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn residual(file: &ModuleFile) -> String {
    let source = file.source();
    let mut out = Vec::new();
    let mut at = 0;
    for call in file.calls() {
        let gap = strip_comments(source.get(at..call.span.start).unwrap_or_default());
        let mut gap = gap.trim_end();
        if let Some(binding) = &call.binding {
            gap = gap
                .strip_suffix('=')
                .map(str::trim_end)
                .and_then(|rest| rest.strip_suffix(binding.as_str()))
                .unwrap_or(gap);
        }
        out.push(words(gap));
        at = call.span.end;
    }
    out.push(words(&strip_comments(source.get(at..).unwrap_or_default())));
    out.retain(|part| !part.is_empty());
    out.join(" ")
}

fn literal(call: &Call, index: usize, key: &str) -> Option<String> {
    match call.positional.get(index).or_else(|| call.arg(key)) {
        Some(Value::Str(literal)) => Some(literal.value.clone()),
        _ => None,
    }
}

fn scan(files: &[ModuleFile]) -> Side {
    let names = Names::of(files, ROOT);
    let mut side = Side::default();
    for file in files {
        let source = file.source();
        side.other
            .push(("top-level statements".to_owned(), residual(file)));
        let mut bindings: BTreeMap<&str, Binding> = BTreeMap::new();
        for call in file.calls() {
            let text =
                |name: &str, skip: usize, keys: &[&str]| render(call, source, name, skip, keys);
            match call.name.as_str() {
                "include" => {}
                "bazel_dep" => match call.str("name") {
                    Some(name) if !call.unpacked => {
                        let version = match call.arg("version") {
                            None => Some(None),
                            Some(Value::Str(literal)) => Some(Some(literal.value.clone())),
                            Some(_) => None,
                        };
                        let dep = match version {
                            Some(version) => Dep {
                                version,
                                rest: text("bazel_dep", 0, &["version"]),
                            },
                            None => Dep {
                                version: None,
                                rest: text("bazel_dep", 0, &[]),
                            },
                        };
                        if side.deps.insert(name.to_owned(), dep).is_some() {
                            side.other
                                .push(("bazel_dep".to_owned(), format!("twice {name}")));
                        }
                    }
                    _ => side
                        .other
                        .push(("bazel_dep".to_owned(), text("bazel_dep", 0, &[]))),
                },
                name if name.ends_with("_override") && !name.contains('.') => {
                    match call.str("module_name") {
                        Some(module) => side
                            .overrides
                            .entry(module.to_owned())
                            .or_default()
                            .push(text(name, 0, &[])),
                        None => side.other.push((name.to_owned(), text(name, 0, &[]))),
                    }
                }
                "use_extension" => {
                    let scope = literal(call, 0, "extension_bzl_file")
                        .zip(literal(call, 1, "extension_name"))
                        .and_then(|(bzl, name)| names.extension(&bzl, &name));
                    let Some(scope) = scope else {
                        side.other
                            .push(("use_extension".to_owned(), text("use_extension", 0, &[])));
                        continue;
                    };
                    side.usages
                        .entry(scope.clone())
                        .or_default()
                        .entries
                        .push(Entry {
                            path: None,
                            text: text(
                                "use_extension",
                                2,
                                &["extension_bzl_file", "extension_name"],
                            ),
                        });
                    if let Some(binding) = &call.binding {
                        bindings.insert(binding, Binding::Extension(scope));
                    }
                }
                "use_repo_rule" => {
                    let Some(rule) = literal(call, 1, "repo_rule_name") else {
                        side.other
                            .push(("use_repo_rule".to_owned(), text("use_repo_rule", 0, &[])));
                        continue;
                    };
                    side.rules
                        .entry(rule.clone())
                        .or_default()
                        .entry(String::new())
                        .or_default()
                        .push(text("use_repo_rule", 0, &[]));
                    if let Some(binding) = &call.binding {
                        bindings.insert(binding, Binding::Rule(rule));
                    }
                }
                "use_repo" | "inject_repo" | "override_repo" => {
                    let usage = match call.positional.first() {
                        Some(Value::Ident(binding)) => match bindings.get(binding.as_str()) {
                            Some(Binding::Extension(scope)) => side.usages.get_mut(scope),
                            _ => None,
                        },
                        _ => None,
                    };
                    let Some(usage) = usage.filter(|_| !call.unpacked) else {
                        side.other
                            .push((call.name.clone(), text(&call.name, 0, &[])));
                        continue;
                    };
                    if call.name != "use_repo" {
                        usage.entries.push(Entry {
                            path: None,
                            text: text(&call.name, 1, &[]),
                        });
                        continue;
                    }
                    for value in call.positional.iter().skip(1) {
                        let mut repo = String::new();
                        render_value(value, &mut repo);
                        usage.repos.insert(repo);
                    }
                    for (alias, value) in &call.named {
                        let mut repo = format!("{alias}=");
                        render_value(value, &mut repo);
                        usage.repos.insert(repo);
                    }
                }
                name => {
                    let (binding, tag) = name.split_once('.').unwrap_or((name, ""));
                    match bindings.get(binding) {
                        Some(Binding::Extension(scope)) if !tag.is_empty() => {
                            let indirect = call.arg("indirect").is_some_and(
                                |value| !matches!(value, Value::Ident(no) if no == "False"),
                            );
                            let path = (GO_PATH_TAGS.contains(&tag) && !indirect)
                                .then(|| call.str("path").map(str::to_owned))
                                .flatten();
                            if let Some(usage) = side.usages.get_mut(scope) {
                                usage.entries.push(Entry {
                                    path,
                                    text: text(tag, 0, &[]),
                                });
                            }
                        }
                        Some(Binding::Rule(rule)) if tag.is_empty() => {
                            let instance = call.str("name").filter(|_| !call.unpacked);
                            let instances = side.rules.entry(rule.clone()).or_default();
                            match instance {
                                Some(instance) => instances
                                    .entry(instance.to_owned())
                                    .or_default()
                                    .push(text(rule, 0, &[])),
                                None => instances.entry(String::new()).or_default().push(text(
                                    rule,
                                    0,
                                    &[],
                                )),
                            }
                        }
                        _ => side.other.push((name.to_owned(), text(name, 0, &[]))),
                    }
                }
            }
        }
    }
    side
}

fn nested(path: &str, other: &str) -> bool {
    path != other
        && (other.starts_with(&format!("{path}/")) || path.starts_with(&format!("{other}/")))
}

fn go_repos(
    base: &Usage,
    head: &Usage,
    known: Option<&BTreeSet<String>>,
) -> Option<BTreeSet<String>> {
    let split = |usage: &Usage| {
        let mut global = Vec::new();
        let mut paths: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for entry in &usage.entries {
            match &entry.path {
                Some(path) => paths
                    .entry(path.clone())
                    .or_default()
                    .push(entry.text.clone()),
                None => global.push(entry.text.clone()),
            }
        }
        (global, paths)
    };
    let (base_global, base_paths) = split(base);
    let (head_global, head_paths) = split(head);
    let usage = |global: Vec<String>| -> BTreeSet<String> {
        global
            .into_iter()
            .map(|text| {
                if text == DEV_USAGE {
                    String::from("use_extension()")
                } else {
                    text
                }
            })
            .collect()
    };
    let (base_global, head_global) = (usage(base_global), usage(head_global));
    if base_global != head_global {
        return None;
    }
    let mut every: BTreeSet<&String> = known?.iter().collect();
    every.extend(base_paths.keys().chain(head_paths.keys()));
    let changed: BTreeSet<&String> = base_paths
        .keys()
        .chain(head_paths.keys())
        .filter(|path| base_paths.get(*path) != head_paths.get(*path))
        .collect();
    if changed
        .iter()
        .any(|path| every.iter().any(|other| nested(path, other)))
    {
        return None;
    }
    Some(changed.into_iter().map(|path| go_repo_name(path)).collect())
}

fn repo_name(entry: &str) -> String {
    let value = entry.split_once('=').map_or(entry, |(_, value)| value);
    value.trim_matches('"').to_owned()
}

fn repo_scope(scope: &Scope, name: String) -> Scope {
    match scope {
        Scope::Extension {
            base,
            name: extension,
        } => Scope::Repo {
            base: base.clone(),
            extension: extension.clone(),
            name,
        },
        Scope::Repo { .. } | Scope::Module(_) => scope.clone(),
    }
}

pub(crate) fn module_diff(
    base: &[ModuleFile],
    head: &[ModuleFile],
    go_paths: Option<&BTreeSet<String>>,
) -> Result<ModuleDiff, dx_error::DxError> {
    let base = scan(base);
    let head = scan(head);
    let mut diff = ModuleDiff::default();
    if let Some((what, _)) = base
        .other
        .iter()
        .zip(&head.other)
        .find(|(before, after)| before != after)
        .map(|(before, _)| before)
    {
        return Err(changes(what));
    }
    if base.other.len() != head.other.len() {
        let longer = if base.other.len() > head.other.len() {
            &base.other
        } else {
            &head.other
        };
        let what = longer
            .get(base.other.len().min(head.other.len()))
            .map_or("MODULE.bazel", |(what, _)| what.as_str());
        return Err(changes(what));
    }
    for name in base.deps.keys().chain(head.deps.keys()) {
        match (base.deps.get(name), head.deps.get(name)) {
            (Some(before), Some(after)) if before == after => {}
            (Some(before), Some(after)) if before.rest == after.rest => {
                diff.deps
                    .insert(name.clone(), after.version.clone().unwrap_or_default());
            }
            (None, Some(after)) => {
                diff.deps
                    .insert(name.clone(), after.version.clone().unwrap_or_default());
            }
            _ => return Err(changes(&format!("bazel_dep {name}"))),
        }
    }
    for module in base.overrides.keys().chain(head.overrides.keys()) {
        if base.overrides.get(module) != head.overrides.get(module) {
            diff.modules.insert(module.clone());
        }
    }
    let empty = Usage::default();
    for scope in base.usages.keys().chain(head.usages.keys()) {
        let before = base.usages.get(scope).unwrap_or(&empty);
        let after = head.usages.get(scope).unwrap_or(&empty);
        if let Some(gone) = before.repos.difference(&after.repos).next() {
            return Err(changes(&format!("use_repo {gone}")));
        }
        diff.scopes.extend(
            after
                .repos
                .difference(&before.repos)
                .map(|entry| repo_scope(scope, repo_name(entry))),
        );
        if before.entries == after.entries {
            continue;
        }
        match go_repos(before, after, go_paths).filter(|_| *scope == go_deps()) {
            Some(repos) => diff
                .scopes
                .extend(repos.into_iter().map(|name| repo_scope(scope, name))),
            None => {
                diff.scopes.insert(scope.clone());
            }
        }
    }
    let no_rules = BTreeMap::new();
    for rule in base.rules.keys().chain(head.rules.keys()) {
        let before = base.rules.get(rule).unwrap_or(&no_rules);
        let after = head.rules.get(rule).unwrap_or(&no_rules);
        for instance in before.keys().chain(after.keys()) {
            match (before.get(instance), after.get(instance)) {
                (Some(old), Some(new)) if old == new => {}
                (Some(_), None) if !instance.is_empty() => {
                    return Err(changes(&format!("{rule} {instance}")));
                }
                _ if instance.is_empty() => {
                    diff.scopes.insert(Scope::extension(ROOT, rule));
                }
                _ => {
                    diff.scopes.insert(Scope::Repo {
                        base: ROOT.to_owned(),
                        extension: rule.clone(),
                        name: instance.clone(),
                    });
                }
            }
        }
    }
    Ok(diff)
}

pub(crate) fn imported(files: &[ModuleFile], module: &str) -> BTreeMap<String, Scope> {
    let names = Names::of(files, module);
    let mut repos = BTreeMap::new();
    for file in files {
        let mut bindings: BTreeMap<&str, Scope> = BTreeMap::new();
        for call in file.calls() {
            match call.name.as_str() {
                "use_extension" => {
                    let scope = literal(call, 0, "extension_bzl_file")
                        .zip(literal(call, 1, "extension_name"))
                        .and_then(|(bzl, name)| names.extension(&bzl, &name));
                    if let (Some(scope), Some(binding)) = (scope, &call.binding) {
                        bindings.insert(binding, scope);
                    }
                }
                "use_repo_rule" => {
                    if let (Some(rule), Some(binding)) =
                        (literal(call, 1, "repo_rule_name"), &call.binding)
                    {
                        bindings.insert(binding, Scope::extension(&module_base(module), &rule));
                    }
                }
                "use_repo" => {
                    let Some(Value::Ident(binding)) = call.positional.first() else {
                        continue;
                    };
                    let Some(scope) = bindings.get(binding.as_str()) else {
                        continue;
                    };
                    for value in call.positional.iter().skip(1) {
                        if let Value::Str(name) = value {
                            repos.insert(name.value.clone(), repo_scope(scope, name.value.clone()));
                        }
                    }
                    for (alias, value) in &call.named {
                        if let Value::Str(name) = value {
                            repos.insert(alias.clone(), repo_scope(scope, name.value.clone()));
                        }
                    }
                }
                name => {
                    if let (Some(scope), Some(instance)) = (bindings.get(name), call.str("name")) {
                        repos.insert(instance.to_owned(), repo_scope(scope, instance.to_owned()));
                    }
                }
            }
        }
    }
    repos
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Named {
    Scope(Scope),
    Module(String),
}

fn main_path(label: &str, package: &str, own: &[String]) -> Option<String> {
    let label = match label.strip_prefix('@') {
        Some(rest) => {
            let (repo, path) = rest.trim_start_matches('@').split_once("//")?;
            if !repo.is_empty() && !own.iter().any(|name| name == repo) {
                return None;
            }
            format!("//{path}")
        }
        None if label.starts_with("//") => label.to_owned(),
        None => format!("//{package}:{}", label.strip_prefix(':')?),
    };
    dx_module_bazel::label_path(&label).ok()
}

fn string_values<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::Str(literal) => out.push(&literal.value),
        Value::List(items) => items.iter().for_each(|item| string_values(item, out)),
        Value::Dict(entries) => entries.iter().for_each(|(key, item)| {
            string_values(key, out);
            string_values(item, out);
        }),
        Value::Ident(_) | Value::Computed { .. } => {}
    }
}

pub(crate) fn named_files(
    files: &[ModuleFile],
    labels: &[Option<String>],
) -> BTreeMap<String, BTreeSet<Named>> {
    let names = Names::of(files, ROOT);
    let mut named: BTreeMap<String, BTreeSet<Named>> = BTreeMap::new();
    for (file, label) in files.iter().zip(labels) {
        let package = label
            .as_deref()
            .and_then(|label| label.strip_prefix("//"))
            .and_then(|label| label.split_once(':'))
            .map_or("", |(package, _)| package);
        let mut bindings: BTreeMap<&str, Binding> = BTreeMap::new();
        for call in file.calls() {
            let target = match call.name.as_str() {
                "use_extension" => {
                    let scope = literal(call, 0, "extension_bzl_file")
                        .zip(literal(call, 1, "extension_name"))
                        .and_then(|(bzl, name)| names.extension(&bzl, &name));
                    if let (Some(scope), Some(binding)) = (scope, &call.binding) {
                        bindings.insert(binding, Binding::Extension(scope));
                    }
                    continue;
                }
                "use_repo_rule" => {
                    if let (Some(rule), Some(binding)) =
                        (literal(call, 1, "repo_rule_name"), &call.binding)
                    {
                        bindings.insert(binding, Binding::Rule(rule));
                    }
                    continue;
                }
                name if name.ends_with("_override") => {
                    let Some(module) = call.str("module_name") else {
                        continue;
                    };
                    Named::Module(module.to_owned())
                }
                name => {
                    let (binding, tag) = name.split_once('.').unwrap_or((name, ""));
                    match (bindings.get(binding), tag) {
                        (Some(Binding::Extension(scope)), tag) if !tag.is_empty() => {
                            Named::Scope(scope.clone())
                        }
                        (Some(Binding::Rule(rule)), "") => Named::Scope(match call.str("name") {
                            Some(instance) => Scope::Repo {
                                base: ROOT.to_owned(),
                                extension: rule.clone(),
                                name: instance.to_owned(),
                            },
                            None => Scope::extension(ROOT, rule),
                        }),
                        _ => continue,
                    }
                }
            };
            let mut values = Vec::new();
            for value in call
                .positional
                .iter()
                .chain(call.named.iter().map(|(_, value)| value))
            {
                string_values(value, &mut values);
            }
            for value in values {
                if let Some(path) = main_path(value, package, &names.own) {
                    named.entry(path).or_default().insert(target.clone());
                }
            }
        }
    }
    named
}

pub(crate) fn covers(changed: &Scope, repo: &Scope) -> bool {
    match repo {
        Scope::Repo {
            base,
            extension,
            name,
        } => changed.matches(&format!("{base}+{extension}+{name}")),
        Scope::Extension { .. } | Scope::Module(_) => changed == repo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "module(name = \"root\")\n\
bazel_dep(name = \"gazelle\", version = \"0.54.0\")\n\
bazel_dep(name = \"rules_go\", version = \"0.60.0\")\n\
go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\n\
go_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n\
use_repo(go_deps, \"com_github_x_y\")\n\
crate = use_extension(\"//rust:extensions.bzl\", \"cargo\")\n\
crate.spec(package = \"serde\", version = \"1\")\n\
use_repo(crate, \"crates\")\n\
http = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\n\
http(name = \"blob\", urls = [\"a\"])\n\
http(name = \"other\", urls = [\"b\"])\n\
register_toolchains(\"//tc:all\")\n";

    fn diff(base: &str, head: &str) -> Result<ModuleDiff, dx_error::DxError> {
        let parse = |text: &str| vec![ModuleFile::parse(text).expect("parses")];
        module_diff(
            &parse(base),
            &parse(head),
            Some(&BTreeSet::from(["github.com/x/y".to_owned()])),
        )
    }

    fn repo(base: &str, extension: &str, name: &str) -> Scope {
        Scope::Repo {
            base: base.to_owned(),
            extension: extension.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn comments_and_formatting_change_nothing() {
        let head = HEAD
            .replace("register_toolchains", "# note\nregister_toolchains")
            .replace("\"0.54.0\")", "\"0.54.0\",  # floor: x\n)");
        assert_eq!(diff(HEAD, &head), Ok(ModuleDiff::default()));
    }

    #[test]
    fn dev_go_deps_usage_selects_only_its_go_repos() {
        let head = format!(
            "{HEAD}dev = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\", dev_dependency = True)\n\
dev.module(path = \"github.com/rhysd/actionlint\", sum = \"h1:a\", version = \"v1.7.13\")\n\
dev.module(path = \"github.com/fatih/color\", indirect = False, sum = \"h1:b\", version = \"v1.19.0\")\n\
use_repo(dev, \"com_github_rhysd_actionlint\")\n"
        );
        let found = diff(HEAD, &head).expect("narrows");
        assert_eq!(
            found.scopes,
            BTreeSet::from([
                repo("gazelle+", "go_deps", "com_github_fatih_color"),
                repo("gazelle+", "go_deps", "com_github_rhysd_actionlint"),
            ])
        );
        assert!(found.deps.is_empty());
    }

    #[test]
    fn indirect_go_deps_modules_select_the_extension() {
        let all = BTreeSet::from([Scope::extension("gazelle+", "go_deps")]);
        for indirect in ["True", "INDIRECT"] {
            let head = format!(
                "{HEAD}go_deps.module(path = \"github.com/fatih/color\", indirect = {indirect}, sum = \"h1:b\", version = \"v1.19.0\")\n"
            );
            assert_eq!(
                diff(HEAD, &head).expect("narrows").scopes,
                all,
                "{indirect}"
            );
        }
    }

    #[test]
    fn global_go_deps_tags_and_nested_paths_select_the_extension() {
        let all = BTreeSet::from([Scope::extension("gazelle+", "go_deps")]);
        for extra in [
            "go_deps.gazelle_default_attributes(directives = [\"x\"])\n",
            "go_deps.module(path = \"github.com/x/y/v2\", sum = \"h1:a\", version = \"v2.0.0\")\n",
            "go_deps.module(path = \"github.com/x\", sum = \"h1:a\", version = \"v1.0.0\")\n",
        ] {
            let head = format!("{HEAD}{extra}");
            assert_eq!(diff(HEAD, &head).expect("narrows").scopes, all, "{extra}");
        }
    }

    #[test]
    fn other_extension_tags_select_their_extension() {
        let head = HEAD.replace("version = \"1\")", "version = \"2\")");
        assert_eq!(
            diff(HEAD, &head).expect("narrows").scopes,
            BTreeSet::from([Scope::extension(ROOT, "cargo")])
        );
    }

    #[test]
    fn repo_rule_instances_select_their_repo() {
        let head = HEAD.replace("urls = [\"a\"]", "urls = [\"c\"]");
        assert_eq!(
            diff(HEAD, &head).expect("narrows").scopes,
            BTreeSet::from([repo(ROOT, "http_archive", "blob")])
        );
    }

    #[test]
    fn version_bumps_added_deps_and_overrides_name_modules() {
        let head = format!(
            "{}bazel_dep(name = \"new\", version = \"1\")\nsingle_version_override(module_name = \"rules_go\", patches = [\"//p:x.patch\"])\n",
            HEAD.replace("0.54.0", "0.55.0")
        );
        let found = diff(HEAD, &head).expect("narrows");
        assert_eq!(
            found.deps,
            BTreeMap::from([
                ("gazelle".to_owned(), "0.55.0".to_owned()),
                ("new".to_owned(), "1".to_owned()),
            ])
        );
        assert_eq!(found.modules, BTreeSet::from(["rules_go".to_owned()]));
    }

    #[test]
    fn unknown_edits_widen() {
        for (head, what) in [
            (HEAD.replace("\"root\"", "\"renamed\""), "module"),
            (HEAD.replace("//tc:all", "//tc2:all"), "register_toolchains"),
            (
                HEAD.replace(", \"com_github_x_y\"", ""),
                "use_repo \"com_github_x_y\"",
            ),
            (
                HEAD.replace("http(name = \"other\", urls = [\"b\"])\n", ""),
                "http_archive other",
            ),
            (
                HEAD.replace("bazel_dep(name = \"rules_go\", version = \"0.60.0\")\n", ""),
                "bazel_dep rules_go",
            ),
            (
                HEAD.replace(
                    "version = \"0.60.0\")",
                    "version = \"0.60.0\", dev_dependency = True)",
                ),
                "bazel_dep rules_go",
            ),
            (format!("{HEAD}X = \"a\"\n"), "top-level statements"),
            (format!("{HEAD}print(\"x\")\n"), "print"),
            (
                format!("{HEAD}e = use_extension(\"@nobody//:e.bzl\", \"e\")\n"),
                "use_extension",
            ),
        ] {
            assert_eq!(diff(HEAD, &head), Err(changes(what)), "{head}");
        }
    }

    #[test]
    fn comments_inside_strings_are_kept() {
        assert_eq!(
            strip_comments("x = \"a#b\" # c\ny = '''q#\"'''# d\n"),
            "x = \"a#b\" \ny = '''q#\"'''\n"
        );
    }

    #[test]
    fn named_files_map_to_their_extension_repo_or_module() {
        let text = format!(
            "{HEAD}crate.annotation(crate = \"x\", patches = [\"@root//rust:x.patch\"])\nhttp(name = \"third\", build_file = \":third.BUILD\")\nsingle_version_override(module_name = \"rules_go\", patches = [\"//p:go.patch\"])\n"
        );
        let files = vec![ModuleFile::parse(&text).expect("parses")];
        let named = named_files(&files, &[Some("//ext:deps.MODULE.bazel".to_owned())]);
        let one = |path: &str| named.get(path).cloned().unwrap_or_default();
        assert_eq!(
            one("rust/x.patch"),
            BTreeSet::from([Named::Scope(Scope::extension(ROOT, "cargo"))])
        );
        assert_eq!(
            one("ext/third.BUILD"),
            BTreeSet::from([Named::Scope(repo(ROOT, "http_archive", "third"))])
        );
        assert_eq!(
            one("p/go.patch"),
            BTreeSet::from([Named::Module("rules_go".to_owned())])
        );
        assert_eq!(
            one("third_party/go/go.mod"),
            BTreeSet::from([Named::Scope(Scope::extension("gazelle+", "go_deps"))])
        );
    }

    #[test]
    fn imported_repos_name_their_canonical_scope() {
        let files = vec![ModuleFile::parse(HEAD).expect("parses")];
        let repos = imported(&files, ROOT);
        assert_eq!(
            repos.get("com_github_x_y"),
            Some(&repo("gazelle+", "go_deps", "com_github_x_y"))
        );
        assert_eq!(repos.get("crates"), Some(&repo(ROOT, "cargo", "crates")));
        assert_eq!(repos.get("blob"), Some(&repo(ROOT, "http_archive", "blob")));
        assert!(covers(
            &Scope::extension("gazelle+", "go_deps"),
            &repo("gazelle+", "go_deps", "com_github_x_y")
        ));
        assert!(!covers(
            &Scope::extension(ROOT, "cargo"),
            &repo("gazelle+", "go_deps", "com_github_x_y")
        ));
    }
}
