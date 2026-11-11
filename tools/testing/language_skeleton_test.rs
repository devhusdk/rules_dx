//! Every language directory follows docs/design/language-skeleton.md outside tools/testing/language_skeleton_allowlist.txt, and the allowlist holds no spare line.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::rc::Rc;

use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstExpr, AstLiteral, AstStmt, ExprP, ParameterP, StmtP,
};
use starlark_syntax::syntax::{AstModule, Dialect};
#[cfg(windows)]
use windows_sys as _;
use {sha2 as _, tempfile as _};

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/testing/language_skeleton_allowlist.txt";

/// Every rule id an allowlist line may name.
const RULES: [&str; 23] = [
    "defs-reexport",
    "dev-proxy-tags",
    "entries",
    "entry-points",
    "export-kind",
    "extension-label",
    "extension-name",
    "fixture-hello",
    "fixture-name",
    "fixture-referenced",
    "membership",
    "name-prefix",
    "pins",
    "platforms-option",
    "platforms-type",
    "private-names",
    "provider-name",
    "readme-headings",
    "register-dev",
    "register-label",
    "test-location",
    "title",
    "wrap",
];

/// Fewer language directories than this means the snapshot lost the tree.
const MIN_LANGS: usize = 10;

/// Entries a language directory may hold.
const ENTRIES: [&str; 8] = [
    "BUILD.bazel",
    "README.md",
    "defs.bzl",
    "extensions.bzl",
    "lang.json",
    "pins.json",
    "private",
    "tests",
];

/// Entries every language directory holds.
const REQUIRED_ENTRIES: [&str; 5] = ["BUILD.bazel", "defs.bzl", "lang.json", "private", "tests"];

/// Directory names `private/` never holds.
const BANNED_PRIVATE: [&str; 5] = ["fixtures", "testdata", "testing", "tests", "toolchains"];

/// Directories of `tests/` that are not under `fixtures/`.
const TEST_DIRS: [&str; 3] = ["conformance", "consumer", "fixtures"];

/// The `##` headings of a language README, in order.
const HEADINGS: [&str; 10] = [
    "Dependencies",
    "Generate",
    "Quality",
    "Coverage",
    "Bench and fuzz",
    "API docs",
    "Packaging",
    "Toolchain",
    "Platforms",
    "License",
];

/// Suffixes of executable macro names.
const EXECUTABLE: [&str; 4] = ["_binary", "_test", "_bench", "_fuzz_test"];

/// The file and the macros every executable macro reaches.
const WRAPPER: &str = "core/wrapper.bzl";
const WRAPS: [&str; 2] = ["dx_wrap_binary", "dx_wrap_test"];

/// Tags that declare a development hub or test dependencies.
const DEV_TAGS: [&str; 6] = ["annotation", "crates", "from_lock", "license", "lock", "provider"];

/// The option only `core/platform.bzl` names.
const PLATFORMS_OPTION: &str = "//command_line_option:platforms";

/// What a top-level name is assigned.
#[derive(Clone)]
enum Rhs {
    Name(String),
    Call(String),
    Value,
}

/// One function: its parameter names and the names its body uses.
#[derive(Default)]
struct Def {
    params: Vec<String>,
    names: BTreeSet<String>,
}

/// What one BUILD or .bzl file declares.
#[derive(Default)]
struct Module {
    loads: BTreeMap<String, (String, String)>,
    assigns: BTreeMap<String, Rhs>,
    defs: BTreeMap<String, Def>,
    calls: BTreeSet<String>,
    platform_attrs: BTreeSet<String>,
    reexports_only: bool,
}

/// Where a name leads after following assignments and loads.
enum Located {
    Def(String, String),
    Call(String),
    Value,
    Outside,
}

/// What an exported name is.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Rule,
    Macro,
    Provider,
    Extension,
    Constant,
    Helper,
    Other,
}

/// One `use_extension` proxy of the root `MODULE.bazel`.
struct Usage {
    proxy: String,
    label: String,
    name: String,
    dev: bool,
}

/// What the root `MODULE.bazel` declares.
#[derive(Default)]
struct RootModule {
    usages: Vec<Usage>,
    registered: Vec<String>,
    registers_dev: bool,
    tags: Vec<(String, String)>,
}

/// Violations keyed by rule id and path, each with what is wrong.
#[derive(Default)]
struct Found(BTreeMap<(String, String), Vec<String>>);

impl Found {
    fn add(&mut self, rule: &str, path: &str, detail: impl Into<String>) {
        assert!(RULES.contains(&rule), "unknown rule id {rule}");
        let detail = detail.into();
        let details = self
            .0
            .entry((rule.to_owned(), path.to_owned()))
            .or_default();
        if !details.contains(&detail) {
            details.push(detail);
        }
    }

    fn lines(&self) -> BTreeSet<String> {
        self.0
            .keys()
            .map(|(rule, path)| format!("{rule} {path}"))
            .collect()
    }
}

fn callee_name(callee: &AstExpr) -> String {
    match &callee.node {
        ExprP::Identifier(ident) => ident.node.ident.clone(),
        ExprP::Dot(_, name) => name.node.clone(),
        _ => String::new(),
    }
}

fn string_literal(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(text)) => Some(text.node.as_str()),
        _ => None,
    }
}

fn is_true(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Identifier(ident) if ident.node.ident == "True")
}

/// The attr kind of `attr.<kind>(...)`.
fn attr_kind(expr: &AstExpr) -> Option<String> {
    let ExprP::Call(callee, _) = &expr.node else {
        return None;
    };
    let ExprP::Dot(object, kind) = &callee.node else {
        return None;
    };
    matches!(&object.node, ExprP::Identifier(ident) if ident.node.ident == "attr")
        .then(|| kind.node.clone())
}

fn rhs_of(expr: &AstExpr) -> Rhs {
    match &expr.node {
        ExprP::Identifier(ident) => Rhs::Name(ident.node.ident.clone()),
        ExprP::Call(callee, _) => Rhs::Call(callee_name(callee)),
        _ => Rhs::Value,
    }
}

/// Walks one file and fills its `Module`.
struct Scanner<'a> {
    module: &'a mut Module,
    def: Option<String>,
}

impl Scanner<'_> {
    fn stmt(&mut self, stmt: &AstStmt, top: bool, first: bool) {
        match &stmt.node {
            StmtP::Statements(stmts) => {
                for (index, child) in stmts.iter().enumerate() {
                    self.stmt(child, top, first && index == 0);
                }
            }
            StmtP::Load(load) => {
                for arg in &load.args {
                    self.module.loads.insert(
                        arg.local.node.ident.clone(),
                        (load.module.node.clone(), arg.their.node.clone()),
                    );
                }
            }
            StmtP::Def(def) => {
                self.module.reexports_only = false;
                let name = def.name.node.ident.clone();
                let params = def
                    .params
                    .iter()
                    .filter_map(|param| match &param.node {
                        ParameterP::Normal(ident, ..) => Some(ident.node.ident.clone()),
                        ParameterP::KwArgs(..) => Some("**".to_owned()),
                        _ => None,
                    })
                    .collect();
                self.module.defs.insert(
                    name.clone(),
                    Def {
                        params,
                        names: BTreeSet::new(),
                    },
                );
                let outer = self.def.replace(name);
                self.stmt(&def.body, false, false);
                self.def = outer;
            }
            StmtP::Assign(assign) => {
                let reexport = matches!(&assign.lhs.node, AssignTargetP::Identifier(_))
                    && matches!(&assign.rhs.node, ExprP::Identifier(_));
                if top {
                    self.module.reexports_only &= reexport;
                    self.assign(&assign.lhs.node, &assign.rhs);
                }
                self.expr(&assign.rhs);
            }
            StmtP::AssignModify(_, _, rhs) => {
                self.module.reexports_only = false;
                self.expr(rhs);
            }
            StmtP::If(condition, body) => {
                self.module.reexports_only = false;
                self.expr(condition);
                self.stmt(body, false, false);
            }
            StmtP::IfElse(condition, branches) => {
                self.module.reexports_only = false;
                self.expr(condition);
                self.stmt(&branches.0, false, false);
                self.stmt(&branches.1, false, false);
            }
            StmtP::For(each) => {
                self.module.reexports_only = false;
                self.expr(&each.over);
                self.stmt(&each.body, false, false);
            }
            StmtP::Expression(expr) => {
                let docstring = top && first && string_literal(expr).is_some();
                if !docstring {
                    self.module.reexports_only = false;
                }
                self.expr(expr);
            }
            StmtP::Return(Some(expr)) => self.expr(expr),
            StmtP::Return(None) | StmtP::Break | StmtP::Continue | StmtP::Pass => {}
        }
    }

    fn assign<P: starlark_syntax::syntax::ast::AstPayload>(
        &mut self,
        target: &AssignTargetP<P>,
        rhs: &AstExpr,
    ) {
        match target {
            AssignTargetP::Identifier(ident) => {
                self.module
                    .assigns
                    .insert(ident.node.ident.clone(), rhs_of(rhs));
            }
            AssignTargetP::Tuple(targets) => {
                let value = match rhs_of(rhs) {
                    Rhs::Call(callee) => Rhs::Call(callee),
                    _ => Rhs::Value,
                };
                for inner in targets {
                    if let AssignTargetP::Identifier(ident) = &inner.node {
                        self.module
                            .assigns
                            .insert(ident.node.ident.clone(), value.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn expr(&mut self, expr: &AstExpr) {
        match &expr.node {
            ExprP::Identifier(ident) => {
                if let Some(def) = &self.def
                    && let Some(entry) = self.module.defs.get_mut(def)
                {
                    entry.names.insert(ident.node.ident.clone());
                }
            }
            ExprP::Call(callee, args) => {
                self.module.calls.insert(callee_name(callee));
                for arg in &args.args {
                    if let ArgumentP::Named(key, value) = &arg.node
                        && key.node == "platforms"
                        && let Some(kind) = attr_kind(value)
                    {
                        self.module.platform_attrs.insert(kind);
                    }
                }
            }
            ExprP::Dict(entries) => {
                for (key, value) in entries {
                    if string_literal(key) == Some("platforms")
                        && let Some(kind) = attr_kind(value)
                    {
                        self.module.platform_attrs.insert(kind);
                    }
                }
            }
            _ => {}
        }
        expr.node.visit_expr(|child| self.expr(child));
    }
}

fn scan_module(path: &str, source: &str) -> Module {
    let ast = AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| panic!("{path}: does not parse: {}", error.without_diagnostic()));
    let mut module = Module {
        reexports_only: true,
        ..Module::default()
    };
    Scanner {
        module: &mut module,
        def: None,
    }
    .stmt(ast.statement(), true, true);
    module
}

fn scan_root_module(source: &str) -> RootModule {
    let ast = AstModule::parse("MODULE.bazel", source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| {
            panic!(
                "MODULE.bazel: does not parse: {}",
                error.without_diagnostic()
            )
        });
    let mut root = RootModule::default();
    let StmtP::Statements(stmts) = &ast.statement().node else {
        root_stmt(ast.statement(), &mut root);
        return root;
    };
    for stmt in stmts {
        root_stmt(stmt, &mut root);
    }
    root
}

fn root_stmt(stmt: &AstStmt, root: &mut RootModule) {
    match &stmt.node {
        StmtP::Assign(assign) => {
            let AssignTargetP::Identifier(proxy) = &assign.lhs.node else {
                return;
            };
            let ExprP::Call(callee, args) = &assign.rhs.node else {
                return;
            };
            if callee_name(callee) != "use_extension" {
                return;
            }
            let mut positional = Vec::new();
            let mut dev = false;
            for arg in &args.args {
                match &arg.node {
                    ArgumentP::Positional(value) => {
                        positional.push(string_literal(value).unwrap_or_default().to_owned());
                    }
                    ArgumentP::Named(key, value) if key.node == "dev_dependency" => {
                        dev = is_true(value);
                    }
                    _ => {}
                }
            }
            if let [label, name] = positional.as_slice() {
                root.usages.push(Usage {
                    proxy: proxy.node.ident.clone(),
                    label: label.clone(),
                    name: name.clone(),
                    dev,
                });
            }
        }
        StmtP::Expression(expr) => {
            let ExprP::Call(callee, args) = &expr.node else {
                return;
            };
            if let ExprP::Dot(object, tag) = &callee.node {
                if let ExprP::Identifier(proxy) = &object.node {
                    root.tags.push((proxy.node.ident.clone(), tag.node.clone()));
                }
                return;
            }
            if callee_name(callee) != "register_toolchains" {
                return;
            }
            for arg in &args.args {
                match &arg.node {
                    ArgumentP::Positional(value) => {
                        root.registered
                            .push(string_literal(value).unwrap_or_default().to_owned());
                    }
                    ArgumentP::Named(key, _) if key.node == "dev_dependency" => {
                        root.registers_dev = true;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn basename(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// The path a rules_dx label names: `//a/b:c.bzl` gives `a/b/c.bzl`.
fn label_path(from: &str, label: &str) -> Option<String> {
    let label = label.strip_prefix("@rules_dx").unwrap_or(label);
    if label.starts_with('@') {
        return None;
    }
    if let Some(rest) = label.strip_prefix("//") {
        return Some(match rest.split_once(':') {
            Some(("", target)) => target.to_owned(),
            Some((package, target)) => format!("{package}/{target}"),
            None => format!("{rest}/{}", basename(rest)),
        });
    }
    let target = label.strip_prefix(':').unwrap_or(label);
    let dir = parent(from);
    Some(if dir.is_empty() {
        target.to_owned()
    } else {
        format!("{dir}/{target}")
    })
}

fn is_build(path: &str) -> bool {
    matches!(basename(path), "BUILD" | "BUILD.bazel")
}

/// Whether a file can name a fixture: a `BUILD`, `.bzl` or `MODULE.bazel` file.
fn names_fixtures(path: &str) -> bool {
    is_build(path) || path.ends_with(".bzl") || basename(path) == "MODULE.bazel"
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The kind a name reads as when its definition is out of reach.
fn kind_by_name(name: &str) -> Kind {
    let first_upper = name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase());
    if !name.chars().any(|letter| letter.is_ascii_lowercase()) {
        Kind::Constant
    } else if first_upper {
        Kind::Provider
    } else {
        Kind::Rule
    }
}

/// The sources the lint reads: every path under a language directory and the text of the snapshot files.
struct Tree {
    paths: BTreeSet<String>,
    text: BTreeMap<String, String>,
    modules: RefCell<BTreeMap<String, Option<Rc<Module>>>>,
}

impl Tree {
    fn new(paths: BTreeSet<String>, text: BTreeMap<String, String>) -> Self {
        Self {
            paths,
            text,
            modules: RefCell::new(BTreeMap::new()),
        }
    }

    fn langs(&self) -> Vec<String> {
        self.text
            .keys()
            .filter_map(|path| path.strip_suffix("/lang.json"))
            .filter(|dir| !dir.contains('/'))
            .map(str::to_owned)
            .collect()
    }

    fn has_file(&self, path: &str) -> bool {
        self.paths.contains(path) || self.text.contains_key(path)
    }

    fn under<'a>(&'a self, dir: &str) -> impl Iterator<Item = &'a String> + use<'a> {
        let prefix = format!("{dir}/");
        self.paths
            .range(prefix.clone()..)
            .take_while(move |path| path.starts_with(&prefix))
    }

    fn has_dir(&self, dir: &str) -> bool {
        self.under(dir).next().is_some()
    }

    /// The entries of `dir`, each with whether it is a directory.
    fn children(&self, dir: &str) -> BTreeMap<String, bool> {
        let skip = dir.len() + 1;
        let mut out = BTreeMap::new();
        for path in self.under(dir) {
            match path[skip..].split_once('/') {
                Some((name, _)) => out.insert(name.to_owned(), true),
                None => out.insert(path[skip..].to_owned(), false),
            };
        }
        out
    }

    /// Every directory below `dir`.
    fn dirs(&self, dir: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for path in self.under(dir) {
            let mut current = parent(path);
            while current.len() > dir.len() && out.insert(current.to_owned()) {
                current = parent(current);
            }
        }
        out
    }

    fn module(&self, path: &str) -> Option<Rc<Module>> {
        if let Some(known) = self.modules.borrow().get(path) {
            return known.clone();
        }
        let module = self
            .text
            .get(path)
            .map(|source| Rc::new(scan_module(path, source)));
        self.modules
            .borrow_mut()
            .insert(path.to_owned(), module.clone());
        module
    }

    fn manifest(&self, lang: &str) -> serde_json::Value {
        let path = format!("{lang}/lang.json");
        serde_json::from_str(&self.text[&path]).unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    fn locate(&self, file: &str, name: &str) -> Located {
        let mut file = file.to_owned();
        let mut name = name.to_owned();
        for _ in 0..32 {
            let Some(module) = self.module(&file) else {
                return Located::Outside;
            };
            if module.defs.contains_key(&name) {
                return Located::Def(file, name);
            }
            if let Some(rhs) = module.assigns.get(&name) {
                match rhs {
                    Rhs::Name(next) => name.clone_from(next),
                    Rhs::Call(callee) => return Located::Call(callee.clone()),
                    Rhs::Value => return Located::Value,
                }
                continue;
            }
            let Some((label, their)) = module.loads.get(&name) else {
                return Located::Outside;
            };
            let Some(next) = label_path(&file, label) else {
                return Located::Outside;
            };
            file = next;
            name.clone_from(their);
        }
        Located::Outside
    }

    fn kind(&self, file: &str, name: &str) -> Kind {
        match self.locate(file, name) {
            Located::Def(file, def) => {
                let module = self.module(&file).expect("located module");
                let params = &module.defs[&def].params;
                if params.iter().any(|param| param == "name" || param == "**") {
                    Kind::Macro
                } else {
                    Kind::Helper
                }
            }
            Located::Call(callee) => match callee.as_str() {
                "rule" => Kind::Rule,
                "macro" => Kind::Macro,
                "provider" => Kind::Provider,
                "module_extension" => Kind::Extension,
                "struct" | "dict" | "depset" | "select" | "Label" => Kind::Constant,
                "aspect" | "repository_rule" | "tag_class" | "transition" => Kind::Other,
                _ => kind_by_name(name),
            },
            Located::Value => Kind::Constant,
            Located::Outside => kind_by_name(name),
        }
    }

    /// The public names of one .bzl file with their kinds.
    fn exports(&self, file: &str) -> BTreeMap<String, Kind> {
        let Some(module) = self.module(file) else {
            return BTreeMap::new();
        };
        module
            .assigns
            .keys()
            .chain(module.defs.keys())
            .filter(|name| !name.starts_with('_'))
            .map(|name| (name.clone(), self.kind(file, name)))
            .collect()
    }

    /// Whether `name` in `file` is a function that reaches a wrapper macro.
    fn wraps(&self, file: &str, name: &str, seen: &mut BTreeSet<(String, String)>) -> bool {
        let Located::Def(file, def) = self.locate(file, name) else {
            return false;
        };
        if file == WRAPPER && WRAPS.contains(&def.as_str()) {
            return true;
        }
        if !seen.insert((file.clone(), def.clone())) {
            return false;
        }
        let module = self.module(&file).expect("located module");
        module.defs[&def]
            .names
            .iter()
            .any(|used| self.wraps(&file, used, seen))
    }
}

fn strings(value: &serde_json::Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn is_callable(kind: Kind) -> bool {
    matches!(kind, Kind::Rule | Kind::Macro)
}

fn check_entries(tree: &Tree, lang: &str, found: &mut Found) {
    let children = tree.children(lang);
    for name in children.keys() {
        if !ENTRIES.contains(&name.as_str()) {
            found.add("entries", &format!("{lang}/{name}"), "not a skeleton entry");
        }
    }
    for name in REQUIRED_ENTRIES {
        if !children.contains_key(name) {
            found.add(
                "entries",
                &format!("{lang}/{name}"),
                "required entry is missing",
            );
        }
    }
}

fn check_private_names(tree: &Tree, lang: &str, found: &mut Found) {
    let private = format!("{lang}/private");
    for dir in tree.dirs(&private) {
        let rest: Vec<&str> = dir[private.len() + 1..].split('/').collect();
        let tool_testdata = matches!(rest.as_slice(), ["tools", _, "testdata"]);
        if BANNED_PRIVATE.contains(&basename(&dir)) && !tool_testdata {
            found.add(
                "private-names",
                &dir,
                "reserved directory name under private/",
            );
        }
    }
    for path in tree.under(&private) {
        let name = basename(path);
        let template = [".BUILD.bazel", ".BUILD", ".build"]
            .iter()
            .any(|suffix| name.len() > suffix.len() && name.ends_with(suffix));
        let placed = parent(path) == private && name.ends_with(".BUILD.bazel");
        if template && !placed {
            found.add(
                "private-names",
                path,
                format!("a repository BUILD template is {private}/<repo>.BUILD.bazel"),
            );
        }
    }
}

fn check_public_names(tree: &Tree, lang: &str, manifest: &serde_json::Value, found: &mut Found) {
    let defs = format!("{lang}/defs.bzl");
    let Some(module) = tree.module(&defs) else {
        return;
    };
    if !module.reexports_only {
        found.add(
            "defs-reexport",
            &defs,
            "holds more than a docstring, load statements and name = _name assignments",
        );
    }
    let exports = tree.exports(&defs);
    let prefix = format!("{lang}_");
    let squashed = lang.replace('_', "");
    for (name, kind) in &exports {
        match kind {
            Kind::Rule | Kind::Macro => {
                if !name.starts_with(&prefix) {
                    found.add(
                        "name-prefix",
                        &defs,
                        format!("{name} does not start with {prefix}"),
                    );
                }
            }
            Kind::Provider => {
                if !name.ends_with("Info") || !name.to_ascii_lowercase().starts_with(&squashed) {
                    found.add(
                        "provider-name",
                        &defs,
                        format!("{name} is not <{squashed}...>Info"),
                    );
                }
            }
            Kind::Constant => found.add("export-kind", &defs, format!("{name} is a constant")),
            Kind::Helper => found.add("export-kind", &defs, format!("{name} is a helper function")),
            Kind::Extension | Kind::Other => {
                found.add(
                    "export-kind",
                    &defs,
                    format!("{name} is no rule, macro or provider"),
                );
            }
        }
        let executable = EXECUTABLE.iter().any(|suffix| name.ends_with(suffix));
        if is_callable(*kind) && executable && !tree.wraps(&defs, name, &mut BTreeSet::new()) {
            found.add(
                "wrap",
                &defs,
                format!("{name} is not a macro that calls dx_wrap_binary or dx_wrap_test"),
            );
        }
    }
    let exported: BTreeSet<String> = exports
        .iter()
        .filter(|(_, kind)| is_callable(**kind))
        .map(|(name, _)| name.clone())
        .collect();
    let listed: BTreeSet<String> = strings(manifest, "entry_points").into_iter().collect();
    if exported != listed {
        let spare: Vec<_> = listed.difference(&exported).collect();
        let missing: Vec<_> = exported.difference(&listed).collect();
        found.add(
            "entry-points",
            &format!("{lang}/lang.json"),
            format!("entry_points not exported: {spare:?}; exported but not listed: {missing:?}"),
        );
    }
}

fn check_manifest(tree: &Tree, lang: &str, manifest: &serde_json::Value, found: &mut Found) {
    let path = format!("{lang}/lang.json");
    if manifest["title"] == manifest["name"] {
        found.add("title", &path, "title equals name");
    }
    let pins = strings(manifest, "pins");
    let has_file = tree.has_file(&format!("{lang}/pins.json"));
    let wanted: &[&str] = if has_file { &["pins.json"] } else { &[] };
    if pins != wanted {
        found.add(
            "pins",
            &path,
            format!("pins is {pins:?}, the directory wants {wanted:?}"),
        );
    }
}

fn check_platforms(tree: &Tree, lang: &str, found: &mut Found) {
    let tests = format!("{lang}/tests/");
    let private = format!("{lang}/private/");
    let sources: Vec<String> = tree
        .text
        .range(format!("{lang}/")..)
        .take_while(|(path, _)| path.starts_with(&format!("{lang}/")))
        .map(|(path, _)| path.clone())
        .filter(|path| !path.starts_with(&tests))
        .collect();
    for path in sources {
        if path.ends_with(".bzl") {
            let module = tree.module(&path).expect("snapshot source");
            for kind in &module.platform_attrs {
                if kind != "string_list" {
                    found.add(
                        "platforms-type",
                        &path,
                        format!("platforms is attr.{kind}, not attr.string_list"),
                    );
                }
            }
        }
        if path.starts_with(&private) && tree.text[&path].contains(PLATFORMS_OPTION) {
            found.add(
                "platforms-option",
                &path,
                format!("names {PLATFORMS_OPTION}"),
            );
        }
    }
}

/// Whether a file name reads as a test.
fn test_file(name: &str) -> bool {
    if name.ends_with(".bzl") {
        return name.ends_with("_tests.bzl");
    }
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    name.starts_with("test_")
        || name.contains(".test.")
        || name.contains(".spec.")
        || ["_test", "_tests", "Test", "Tests"]
            .iter()
            .any(|suffix| stem.ends_with(suffix))
}

fn test_callee(name: &str) -> bool {
    name.ends_with("_test") || name.ends_with("_tests") || name.ends_with("test_suite")
}

fn check_test_location(tree: &Tree, lang: &str, found: &mut Found) {
    let build = format!("{lang}/tests/BUILD.bazel");
    if tree.has_dir(&format!("{lang}/tests")) && !tree.has_file(&build) {
        found.add("test-location", &build, "missing");
    }
    let private = format!("{lang}/private");
    for path in tree.under(&private) {
        let rest: Vec<&str> = path[private.len() + 1..].split('/').collect();
        if matches!(rest.as_slice(), ["tools", _, ..]) {
            continue;
        }
        if is_build(path) {
            let module = tree.module(path).expect("snapshot source");
            let tests: Vec<_> = module
                .calls
                .iter()
                .filter(|name| test_callee(name))
                .collect();
            if !tests.is_empty() {
                found.add("test-location", path, format!("declares tests: {tests:?}"));
            }
        } else if test_file(basename(path)) {
            found.add("test-location", path, "a test file under private/");
        }
    }
}

fn fixture_name(name: &str) -> bool {
    name.starts_with(|first: char| first.is_ascii_lowercase())
        && name
            .chars()
            .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '_')
}

/// Fixture directories named by a build file outside themselves.
fn referenced_fixtures(tree: &Tree) -> BTreeSet<String> {
    const MARK: &str = "/tests/fixtures/";
    let mut out = BTreeSet::new();
    for (path, text) in &tree.text {
        if !names_fixtures(path) {
            continue;
        }
        let text = text
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .replace(':', "/");
        let bytes = text.as_bytes();
        let mut add = |lang: &str, name: &str, end: usize| {
            let dotted = bytes.get(end) == Some(&b'.');
            let dir = format!("{lang}{MARK}{name}");
            if !lang.is_empty()
                && !name.is_empty()
                && !dotted
                && !path.starts_with(&format!("{dir}/"))
            {
                out.insert(dir);
            }
        };
        for (at, _) in text.match_indices(MARK) {
            let start = bytes[..at]
                .iter()
                .rposition(|byte| !is_word(*byte))
                .map_or(0, |index| index + 1);
            let after = at + MARK.len();
            let end = bytes[after..]
                .iter()
                .position(|byte| !is_word(*byte) && *byte != b'-')
                .map_or(bytes.len(), |index| after + index);
            add(&text[start..at], &text[after..end], end);
        }
        let dir = parent(path);
        let Some(lang) = dir
            .strip_suffix("/tests")
            .filter(|lang| !lang.contains('/'))
        else {
            continue;
        };
        for quote in ["\"fixtures/", "'fixtures/"] {
            for (at, _) in text.match_indices(quote) {
                let after = at + quote.len();
                let end = bytes[after..]
                    .iter()
                    .position(|byte| !is_word(*byte) && *byte != b'-')
                    .map_or(bytes.len(), |index| after + index);
                add(lang, &text[after..end], end);
            }
        }
    }
    out
}

fn check_fixtures(
    tree: &Tree,
    lang: &str,
    manifest: &serde_json::Value,
    referenced: &BTreeSet<String>,
    found: &mut Found,
) {
    let tests = format!("{lang}/tests");
    if !tree.has_dir(&tests) {
        return;
    }
    for (name, is_dir) in tree.children(&tests) {
        if is_dir && !TEST_DIRS.contains(&name.as_str()) {
            found.add(
                "fixture-name",
                &format!("{tests}/{name}"),
                format!("a fixture lives in {tests}/fixtures/<name>/"),
            );
        }
    }
    let fixtures = format!("{tests}/fixtures");
    for (name, is_dir) in tree.children(&fixtures) {
        let path = format!("{fixtures}/{name}");
        if !is_dir {
            found.add("fixture-name", &path, "a file outside a fixture directory");
            continue;
        }
        if !fixture_name(&name) {
            found.add("fixture-name", &path, "not lower-case with underscores");
        }
        if !referenced.contains(&path) {
            found.add(
                "fixture-referenced",
                &path,
                "no BUILD.bazel, .bzl or MODULE.bazel file outside it names the path",
            );
        }
    }
    let conformance = format!("{tests}/conformance/BUILD.bazel");
    if !tree.has_file(&conformance) {
        found.add("fixture-hello", &conformance, "missing");
    }
    let consumer = format!("{tests}/consumer");
    if !strings(manifest, "entry_points").is_empty() && !tree.has_dir(&consumer) {
        found.add(
            "fixture-hello",
            &consumer,
            "missing, and the manifest has entry_points",
        );
    }
}

fn check_readme(tree: &Tree, lang: &str, manifest: &serde_json::Value, found: &mut Found) {
    let path = format!("{lang}/README.md");
    let Some(text) = tree.text.get(&path) else {
        return;
    };
    let title = format!("# {}", manifest["title"].as_str().unwrap_or_default());
    if text.lines().next() != Some(title.as_str()) {
        found.add(
            "readme-headings",
            &path,
            format!("first line is not {title:?}"),
        );
    }
    let mut fenced = false;
    let mut next = 0;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let Some(heading) = line.strip_prefix("## ").filter(|_| !fenced) else {
            continue;
        };
        match HEADINGS[next..]
            .iter()
            .position(|known| *known == heading.trim())
        {
            Some(index) => next += index + 1,
            None => found.add(
                "readme-headings",
                &path,
                format!("## {heading} is not a listed heading in order"),
            ),
        }
    }
}

fn check_extensions(
    tree: &Tree,
    lang: &str,
    manifest: &serde_json::Value,
    root: &RootModule,
    found: &mut Found,
) {
    let file = format!("{lang}/extensions.bzl");
    let ecosystems = strings(manifest, "ecosystems");
    let prefix = format!("{lang}_");
    let named = |name: &str| {
        name == lang || name.starts_with(&prefix) || ecosystems.iter().any(|id| id == name)
    };
    let mut extensions = 0;
    for (name, kind) in tree.exports(&file) {
        if kind != Kind::Extension {
            continue;
        }
        extensions += 1;
        if !named(&name) {
            found.add(
                "extension-name",
                &file,
                format!("{name} is not {lang}, {prefix}<name> or an ecosystem id"),
            );
        }
    }
    for usage in &root.usages {
        let Some(path) = label_path("MODULE.bazel", &usage.label) else {
            continue;
        };
        if !path.starts_with(&format!("{lang}/")) {
            continue;
        }
        if path == file {
            extensions += 1;
        } else {
            found.add(
                "extension-label",
                &path,
                format!(
                    "use_extension names {}, not //{lang}:extensions.bzl",
                    usage.label
                ),
            );
        }
        if !named(&usage.name) {
            found.add(
                "extension-name",
                &path,
                format!(
                    "{} is not {lang}, {prefix}<name> or an ecosystem id",
                    usage.name
                ),
            );
        }
        if !usage.dev {
            continue;
        }
        for (proxy, tag) in &root.tags {
            if *proxy == usage.proxy && !DEV_TAGS.contains(&tag.as_str()) {
                found.add(
                    "dev-proxy-tags",
                    &path,
                    format!("dev_dependency usage {proxy} calls {tag}"),
                );
            }
        }
    }
    let callables = tree
        .exports(&format!("{lang}/defs.bzl"))
        .values()
        .filter(|kind| is_callable(**kind))
        .count();
    if callables + extensions == 0 {
        found.add(
            "membership",
            lang,
            "exports no rule, macro or module extension",
        );
    }
}

fn check_root_module(langs: &[String], root: &RootModule, found: &mut Found) {
    for usage in &root.usages {
        let Some(path) = label_path("MODULE.bazel", &usage.label) else {
            continue;
        };
        let top = path.split('/').next().unwrap_or_default();
        if langs.iter().any(|lang| lang == top) {
            continue;
        }
        let private = path.split('/').any(|part| part == "private");
        if basename(&path) != "extensions.bzl" || private {
            found.add(
                "extension-label",
                &path,
                format!(
                    "use_extension names {}, not //<dir>:extensions.bzl",
                    usage.label
                ),
            );
        }
    }
    for label in &root.registered {
        if let Some(rest) = label.strip_prefix("//") {
            let package = rest.split_once(':').map_or(rest, |(package, _)| package);
            found.add(
                "register-label",
                package,
                format!("register_toolchains takes {label}"),
            );
        }
    }
    if root.registers_dev {
        found.add(
            "register-dev",
            "MODULE.bazel",
            "register_toolchains sets dev_dependency",
        );
    }
}

/// Every violation in `tree`.
fn lint(tree: &Tree) -> Found {
    let mut found = Found::default();
    let langs = tree.langs();
    let root = tree
        .text
        .get("MODULE.bazel")
        .map(|source| scan_root_module(source))
        .unwrap_or_default();
    let referenced = referenced_fixtures(tree);
    check_root_module(&langs, &root, &mut found);
    for lang in &langs {
        let manifest = tree.manifest(lang);
        check_entries(tree, lang, &mut found);
        check_private_names(tree, lang, &mut found);
        check_public_names(tree, lang, &manifest, &mut found);
        check_manifest(tree, lang, &manifest, &mut found);
        check_platforms(tree, lang, &mut found);
        check_extensions(tree, lang, &manifest, &root, &mut found);
        check_test_location(tree, lang, &mut found);
        check_fixtures(tree, lang, &manifest, &referenced, &mut found);
        check_readme(tree, lang, &manifest, &mut found);
    }
    found
}

/// The lines of the allowlist and what is wrong with its form.
fn parse_allowlist(text: &str) -> (BTreeSet<String>, Vec<String>) {
    let mut lines = BTreeSet::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let well_formed =
            matches!(fields.as_slice(), [rule, path] if RULES.contains(rule) && !path.is_empty());
        if !well_formed {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        if previous.is_some_and(|before| before >= line) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(line);
        lines.insert(line.to_owned());
    }
    (lines, problems)
}

/// What differs between the violations and the allowlist.
fn differences(found: &Found, allowed: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for ((rule, path), details) in &found.0 {
        if !allowed.contains(&format!("{rule} {path}")) {
            out.push(format!("{rule} {path}: {}; fix it", details.join("; ")));
        }
    }
    let lines = found.lines();
    for line in allowed.difference(&lines) {
        out.push(format!(
            "{line}: no such violation; delete the line from {ALLOWLIST}"
        ));
    }
    out
}

fn collect(root: &Path, dir: &Path, text: &mut BTreeMap<String, String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{dir:?}: {error}"));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("{dir:?}: {error}"))
            .path();
        if path.is_dir() {
            collect(root, &path, text);
        } else {
            let rel = path.strip_prefix(root).expect("under the snapshot root");
            text.insert(
                rel.to_string_lossy().replace('\\', "/"),
                dx_testing::read_text(&path),
            );
        }
    }
}

fn snapshot_tree() -> Tree {
    let root = dx_testing::source_snapshot();
    let mut text = BTreeMap::new();
    collect(&root, &root, &mut text);
    let key = dx_testing::env_text("DX_PATHS_JSON");
    let listing =
        dx_testing::rlocation(key.trim()).unwrap_or_else(|error| panic!("DX_PATHS_JSON: {error}"));
    let paths: BTreeSet<String> = serde_json::from_str(&dx_testing::read_text(&listing))
        .unwrap_or_else(|error| panic!("DX_PATHS_JSON: {error}"));
    Tree::new(paths, text)
}

#[test]
fn every_language_directory_follows_the_skeleton_or_is_allowlisted() {
    let tree = snapshot_tree();
    let langs = tree.langs().len();
    assert!(
        langs >= MIN_LANGS,
        "only {langs} language directories in the snapshot"
    );
    let key = dx_testing::env_text("LANGUAGE_SKELETON_ALLOWLIST");
    let (allowed, mut problems) = parse_allowlist(&dx_testing::read_runfiles(key.trim()));
    problems.extend(differences(&lint(&tree), &allowed));
    assert!(
        problems.is_empty(),
        "{} language skeleton problems against {ALLOWLIST}; rules: docs/design/language-skeleton.md\n{}",
        problems.len(),
        problems.join("\n")
    );
}

const TOY_MANIFEST: &str = r#"{
  "name": "toy",
  "title": "Toy",
  "ecosystems": ["toyhub"],
  "pins": ["pins.json"],
  "entry_points": ["toy_binary", "toy_library", "toy_test"]
}"#;

const TOY_DEFS: &str = r#""""Public Toy API."""

load(
    "//toy/private:defs.bzl",
    _ToyInfo = "ToyInfo",
    _toy_binary = "toy_binary",
    _toy_library = "toy_library",
    _toy_test = "toy_test",
)

ToyInfo = _ToyInfo
toy_binary = _toy_binary
toy_library = _toy_library
toy_test = _toy_test
"#;

const TOY_PRIVATE_DEFS: &str = r#""""Toy rules."""

load("//core:wrapper.bzl", "dx_wrap_binary", _wrap_test = "dx_wrap_test")

ToyInfo = provider(fields = ["srcs"])

def _impl(ctx):
    return [ToyInfo(srcs = ctx.files.srcs)]

toy_library = rule(
    implementation = _impl,
    attrs = {
        "srcs": attr.label_list(),
        "platforms": attr.string_list(),
    },
)

def toy_binary(name, **kwargs):
    dx_wrap_binary(name = name, **kwargs)

def _checked(name, **kwargs):
    _wrap_test(name = name, **kwargs)

def toy_test(name, **kwargs):
    _checked(name, **kwargs)
"#;

const TOY_EXTENSIONS: &str = r#""""Public Toy module extensions."""

load("//toy/private/toolchain:extensions.bzl", _toy = "toy", _toyhub = "toyhub")

toy = _toy
toyhub = _toyhub
"#;

const TOY_PRIVATE_EXTENSIONS: &str = r#""""Toy module extensions."""

def _impl(_mctx):
    pass

toy = module_extension(implementation = _impl)
toyhub = module_extension(implementation = _impl)
"#;

const TOY_MODULE: &str = r#"module(name = "rules_dx")

toy = use_extension("//toy:extensions.bzl", "toy")
use_repo(toy, "dx_toy_toolchains")

toy_dev = use_extension("//toy:extensions.bzl", "toyhub", dev_dependency = True)
toy_dev.lock(name = "toy_fixture")

register_toolchains("@dx_toy_toolchains//:all")
"#;

const TOY_README: &str =
    "# Toy\n\n## Dependencies\n\n```\n## not a heading\n```\n\n### Extra\n\n## Toolchain\n";

/// A language directory that follows the page.
fn toy() -> BTreeMap<String, String> {
    [
        ("MODULE.bazel", TOY_MODULE),
        (
            "core/wrapper.bzl",
            "def dx_wrap_binary(name, **kwargs):\n    pass\n\ndef dx_wrap_test(name, **kwargs):\n    pass\n",
        ),
        ("toy/BUILD.bazel", "toolchain_type(name = \"toolchain_type\")\n"),
        ("toy/README.md", TOY_README),
        ("toy/defs.bzl", TOY_DEFS),
        ("toy/extensions.bzl", TOY_EXTENSIONS),
        ("toy/lang.json", TOY_MANIFEST),
        ("toy/pins.json", "{}"),
        ("toy/private/BUILD.bazel", ""),
        ("toy/private/defs.bzl", TOY_PRIVATE_DEFS),
        ("toy/private/toy_sdk.BUILD.bazel", ""),
        ("toy/private/toolchain/BUILD.bazel", ""),
        ("toy/private/toolchain/extensions.bzl", TOY_PRIVATE_EXTENSIONS),
        (
            "toy/private/tools/gen/BUILD.bazel",
            "rust_test(name = \"gen_test\", srcs = [\"gen_test.rs\"])\n",
        ),
        ("toy/private/tools/gen/gen_test.rs", ""),
        ("toy/private/tools/gen/testdata/in.txt", ""),
        (
            "toy/tests/BUILD.bazel",
            "filegroup(name = \"app\", srcs = [\"fixtures/app/main.toy\"])\n",
        ),
        ("toy/tests/conformance/BUILD.bazel", ""),
        ("toy/tests/consumer/MODULE.bazel", ""),
        ("toy/tests/defs_tests.bzl", ""),
        ("toy/tests/fixtures/app/main.toy", ""),
        ("toy/tests/fixtures/lib_two/BUILD.bazel", ""),
        (
            "toy/tests/toolchain_tests.bzl",
            "LIB = \"//toy/tests/fixtures/lib_two:lib\"\n",
        ),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.to_owned()))
    .collect()
}

fn lines_of(files: &BTreeMap<String, String>) -> BTreeSet<String> {
    let tree = Tree::new(files.keys().cloned().collect(), files.clone());
    lint(&tree).lines()
}

/// Lints the toy language after `change` and returns the lines of `rule`.
fn broken(rule: &str, change: impl FnOnce(&mut BTreeMap<String, String>)) -> Vec<String> {
    let mut files = toy();
    change(&mut files);
    let prefix = format!("{rule} ");
    lines_of(&files)
        .into_iter()
        .filter(|line| line.starts_with(&prefix))
        .collect()
}

fn put(files: &mut BTreeMap<String, String>, path: &str, text: &str) {
    files.insert(path.to_owned(), text.to_owned());
}

fn swap(files: &mut BTreeMap<String, String>, path: &str, from: &str, to: &str) {
    let text = files
        .get_mut(path)
        .unwrap_or_else(|| panic!("{path} is a toy file"));
    assert!(text.contains(from), "{path} holds {from:?}");
    *text = text.replace(from, to);
}

fn append(files: &mut BTreeMap<String, String>, path: &str, more: &str) {
    files
        .get_mut(path)
        .unwrap_or_else(|| panic!("{path} is a toy file"))
        .push_str(more);
}

#[test]
fn a_language_directory_that_follows_the_page_passes_every_rule() {
    assert_eq!(lines_of(&toy()), BTreeSet::new());
}

#[test]
fn entries_names_a_stray_and_a_missing_entry() {
    let found = broken("entries", |files| {
        put(files, "toy/repos.bzl", "");
        files.remove("toy/BUILD.bazel");
    });
    assert_eq!(found, ["entries toy/BUILD.bazel", "entries toy/repos.bzl"]);
}

#[test]
fn private_names_rejects_reserved_directories_and_misplaced_templates() {
    let found = broken("private-names", |files| {
        put(files, "toy/private/toolchains/BUILD.bazel", "");
        put(files, "toy/private/quality/testdata/in.txt", "");
        put(files, "toy/private/toolchain/sdk.BUILD.bazel", "");
    });
    assert_eq!(
        found,
        [
            "private-names toy/private/quality/testdata",
            "private-names toy/private/toolchain/sdk.BUILD.bazel",
            "private-names toy/private/toolchains",
        ]
    );
}

#[test]
fn defs_reexport_rejects_a_definition_in_defs_bzl() {
    let found = broken("defs-reexport", |files| {
        append(files, "toy/defs.bzl", "_FLAGS = [\"-O2\"]\n");
    });
    assert_eq!(found, ["defs-reexport toy/defs.bzl"]);
}

#[test]
fn export_kind_rejects_a_constant_and_a_helper() {
    for (private, name) in [
        ("TOY_FLAGS = [\"-O2\"]\n", "TOY_FLAGS"),
        ("def toy_flags(mode):\n    return [mode]\n", "toy_flags"),
    ] {
        let found = broken("export-kind", |files| {
            append(files, "toy/private/defs.bzl", private);
            swap(
                files,
                "toy/defs.bzl",
                ")\n\n",
                &format!("    _{name} = \"{name}\",\n)\n\n"),
            );
            append(files, "toy/defs.bzl", &format!("{name} = _{name}\n"));
        });
        assert_eq!(found, ["export-kind toy/defs.bzl"], "{name}");
    }
}

#[test]
fn name_prefix_rejects_a_rule_named_for_another_directory() {
    let found = broken("name-prefix", |files| {
        swap(
            files,
            "toy/defs.bzl",
            "toy_library = _toy_library",
            "game_library = _toy_library",
        );
    });
    assert_eq!(found, ["name-prefix toy/defs.bzl"]);
}

#[test]
fn provider_name_rejects_a_provider_not_named_for_the_directory() {
    for name in ["GameInfo", "ToyProvider"] {
        let found = broken("provider-name", |files| {
            swap(
                files,
                "toy/defs.bzl",
                "ToyInfo = _ToyInfo",
                &format!("{name} = _ToyInfo"),
            );
        });
        assert_eq!(found, ["provider-name toy/defs.bzl"], "{name}");
    }
}

#[test]
fn entry_points_rejects_a_manifest_that_differs_from_the_exports() {
    let found = broken("entry-points", |files| {
        swap(files, "toy/lang.json", "\"toy_library\", ", "");
    });
    assert_eq!(found, ["entry-points toy/lang.json"]);
}

#[test]
fn title_rejects_a_title_equal_to_the_name() {
    let found = broken("title", |files| {
        swap(files, "toy/lang.json", "\"Toy\"", "\"toy\"");
    });
    assert_eq!(found, ["title toy/lang.json"]);
}

#[test]
fn pins_rejects_another_pin_file_and_an_unlisted_one() {
    let elsewhere = broken("pins", |files| {
        swap(
            files,
            "toy/lang.json",
            "\"pins.json\"",
            "\"private/toolchain/pins.json\"",
        );
    });
    assert_eq!(elsewhere, ["pins toy/lang.json"]);
    let unlisted = broken("pins", |files| {
        swap(files, "toy/lang.json", "[\"pins.json\"]", "[]");
    });
    assert_eq!(unlisted, ["pins toy/lang.json"]);
    let none = broken("pins", |files| {
        swap(files, "toy/lang.json", "[\"pins.json\"]", "[]");
        files.remove("toy/pins.json");
    });
    assert_eq!(none, [""; 0]);
}

#[test]
fn wrap_rejects_an_executable_that_does_not_reach_the_wrapper() {
    let unwrapped = broken("wrap", |files| {
        swap(
            files,
            "toy/private/defs.bzl",
            "_wrap_test(name = name, **kwargs)",
            "native.sh_test(name = name, **kwargs)",
        );
    });
    assert_eq!(unwrapped, ["wrap toy/defs.bzl"]);
    let passed_on = broken("wrap", |files| {
        swap(
            files,
            "toy/private/defs.bzl",
            "    _checked(name, **kwargs)",
            "    _run(name, _checked, kwargs)",
        );
        swap(
            files,
            "toy/private/defs.bzl",
            "load(\"//core:wrapper.bzl\"",
            "load(\"@rules_dx//core:wrapper.bzl\"",
        );
    });
    assert_eq!(passed_on, [""; 0]);
    let rule = broken("wrap", |files| {
        swap(
            files,
            "toy/private/defs.bzl",
            "def toy_binary(name, **kwargs):\n    dx_wrap_binary(name = name, **kwargs)",
            "toy_binary = rule(implementation = _impl, executable = True)",
        );
    });
    assert_eq!(rule, ["wrap toy/defs.bzl"]);
    let lookalike = broken("wrap", |files| {
        swap(
            files,
            "toy/private/defs.bzl",
            "load(\"//core:wrapper.bzl\", \"dx_wrap_binary\", _wrap_test = \"dx_wrap_test\")",
            "load(\":wrapper.bzl\", \"dx_wrap_binary\", _wrap_test = \"dx_wrap_test\")",
        );
        put(
            files,
            "toy/private/wrapper.bzl",
            "def dx_wrap_binary(name, **kwargs):\n    pass\n\ndef dx_wrap_test(name, **kwargs):\n    pass\n",
        );
    });
    assert_eq!(lookalike, ["wrap toy/defs.bzl"]);
}

#[test]
fn platforms_type_rejects_a_platforms_attr_that_is_no_string_list() {
    let found = broken("platforms-type", |files| {
        swap(
            files,
            "toy/private/defs.bzl",
            "\"platforms\": attr.string_list()",
            "\"platforms\": attr.label_list()",
        );
    });
    assert_eq!(found, ["platforms-type toy/private/defs.bzl"]);
}

#[test]
fn platforms_option_rejects_the_option_name_under_private() {
    let found = broken("platforms-option", |files| {
        put(
            files,
            "toy/private/transition.bzl",
            "OUTPUTS = [\"//command_line_option:platforms\"]\n",
        );
        put(
            files,
            "toy/tests/transition_tests.bzl",
            "OUTPUTS = [\"//command_line_option:platforms\"]\n",
        );
    });
    assert_eq!(found, ["platforms-option toy/private/transition.bzl"]);
}

#[test]
fn extension_label_rejects_a_label_that_is_not_the_public_file() {
    let found = broken("extension-label", |files| {
        append(
            files,
            "MODULE.bazel",
            "toy_sdk = use_extension(\"//toy/private/toolchain:extensions.bzl\", \"toy\")\n",
        );
        append(
            files,
            "MODULE.bazel",
            "hub = use_extension(\"//tools/hub:extension.bzl\", \"hub\")\n",
        );
        append(
            files,
            "MODULE.bazel",
            "pins = use_extension(\"//deploy/private/pins:extensions.bzl\", \"pins\")\n",
        );
        append(
            files,
            "MODULE.bazel",
            "core = use_extension(\"//core:extensions.bzl\", \"dx_pins\")\n",
        );
        append(
            files,
            "MODULE.bazel",
            "go = use_extension(\"@rules_go//go:extensions.bzl\", \"go_sdk\")\n",
        );
    });
    assert_eq!(
        found,
        [
            "extension-label deploy/private/pins/extensions.bzl",
            "extension-label tools/hub/extension.bzl",
            "extension-label toy/private/toolchain/extensions.bzl",
        ]
    );
}

#[test]
fn extension_name_rejects_a_name_that_is_no_language_or_ecosystem() {
    let exported = broken("extension-name", |files| {
        swap(
            files,
            "toy/extensions.bzl",
            "toyhub = _toyhub",
            "sdk = _toyhub",
        );
    });
    assert_eq!(exported, ["extension-name toy/extensions.bzl"]);
    let prefixed = broken("extension-name", |files| {
        swap(
            files,
            "toy/extensions.bzl",
            "toyhub = _toyhub",
            "toy_sdk = _toyhub",
        );
    });
    assert_eq!(prefixed, [""; 0]);
    let used = broken("extension-name", |files| {
        append(
            files,
            "MODULE.bazel",
            "sdk = use_extension(\"//toy:extensions.bzl\", \"sdk\")\n",
        );
    });
    assert_eq!(used, ["extension-name toy/extensions.bzl"]);
}

#[test]
fn register_label_rejects_a_workspace_label() {
    let found = broken("register-label", |files| {
        append(
            files,
            "MODULE.bazel",
            "register_toolchains(\"//toy/private/toolchain:all\")\n",
        );
    });
    assert_eq!(found, ["register-label toy/private/toolchain"]);
}

#[test]
fn register_dev_rejects_dev_dependency_on_register_toolchains() {
    let found = broken("register-dev", |files| {
        append(
            files,
            "MODULE.bazel",
            "register_toolchains(\"@dx_toy_dev//:all\", dev_dependency = True)\n",
        );
    });
    assert_eq!(found, ["register-dev MODULE.bazel"]);
}

#[test]
fn dev_proxy_tags_rejects_a_toolchain_tag_on_a_development_usage() {
    let found = broken("dev-proxy-tags", |files| {
        append(
            files,
            "MODULE.bazel",
            "toy_dev.configure(version = \"1\")\ntoy.configure(version = \"1\")\n",
        );
    });
    assert_eq!(found, ["dev-proxy-tags toy/extensions.bzl"]);
}

#[test]
fn dev_proxy_tags_accepts_test_dependency_tags() {
    let found = broken("dev-proxy-tags", |files| {
        append(
            files,
            "MODULE.bazel",
            "toy_dev.from_lock(name = \"x\")\ntoy_dev.crates(name = \"y\")\n",
        );
    });
    assert!(found.is_empty());
}

#[test]
fn test_location_rejects_tests_under_private() {
    let found = broken("test-location", |files| {
        put(files, "toy/private/parse_test.rs", "");
        put(
            files,
            "toy/private/quality/BUILD.bazel",
            "toy_test(name = \"lint_test\")\n",
        );
        put(files, "toy/private/quality/lint_test.bzl", "");
        put(files, "toy/private/quality/lint_tests.bzl", "");
        files.remove("toy/tests/BUILD.bazel");
    });
    assert_eq!(
        found,
        [
            "test-location toy/private/parse_test.rs",
            "test-location toy/private/quality/BUILD.bazel",
            "test-location toy/private/quality/lint_tests.bzl",
            "test-location toy/tests/BUILD.bazel",
        ]
    );
}

#[test]
fn fixture_name_rejects_fixtures_outside_a_named_fixture_directory() {
    let found = broken("fixture-name", |files| {
        put(files, "toy/tests/fixtures/toy-app/BUILD.bazel", "");
        put(files, "toy/tests/fixtures/loose.toy", "");
        put(files, "toy/tests/golden/out.txt", "");
    });
    assert_eq!(
        found,
        [
            "fixture-name toy/tests/fixtures/loose.toy",
            "fixture-name toy/tests/fixtures/toy-app",
            "fixture-name toy/tests/golden",
        ]
    );
}

#[test]
fn fixture_hello_rejects_a_language_without_conformance_or_consumer_fixtures() {
    let found = broken("fixture-hello", |files| {
        files.remove("toy/tests/conformance/BUILD.bazel");
        files.remove("toy/tests/consumer/MODULE.bazel");
    });
    assert_eq!(
        found,
        [
            "fixture-hello toy/tests/conformance/BUILD.bazel",
            "fixture-hello toy/tests/consumer",
        ]
    );
}

#[test]
fn fixture_referenced_rejects_a_fixture_no_build_file_names() {
    let found = broken("fixture-referenced", |files| {
        put(
            files,
            "toy/tests/fixtures/orphan/BUILD.bazel",
            "LABEL = \"//toy/tests/fixtures/orphan:all\"\n",
        );
        put(files, "toy/tests/fixtures/app_two/main.toy", "");
        put(files, "toy/tests/fixtures/readme_only/main.toy", "");
        append(files, "toy/README.md", "toy/tests/fixtures/readme_only\n");
        put(files, "toy/tests/fixtures/comment_only/main.toy", "");
        append(
            files,
            "toy/BUILD.bazel",
            "# gazelle:exclude toy/tests/fixtures/comment_only\n",
        );
    });
    assert_eq!(
        found,
        [
            "fixture-referenced toy/tests/fixtures/app_two",
            "fixture-referenced toy/tests/fixtures/comment_only",
            "fixture-referenced toy/tests/fixtures/orphan",
            "fixture-referenced toy/tests/fixtures/readme_only",
        ]
    );
}

#[test]
fn readme_headings_rejects_a_wrong_title_heading_and_order() {
    for text in [
        "# toy\n",
        "# Toy\n\n## Install\n",
        "# Toy\n\n## Toolchain\n\n## Dependencies\n",
    ] {
        let found = broken("readme-headings", |files| put(files, "toy/README.md", text));
        assert_eq!(found, ["readme-headings toy/README.md"], "{text:?}");
    }
}

#[test]
fn membership_rejects_a_directory_that_exports_nothing() {
    let found = broken("membership", |files| {
        put(files, "toy/defs.bzl", "\"\"\"Public Toy API.\"\"\"\n");
        put(files, "MODULE.bazel", "");
        files.remove("toy/extensions.bzl");
    });
    assert_eq!(found, ["membership toy"]);
    let extension_only = broken("membership", |files| {
        put(files, "toy/defs.bzl", "\"\"\"Public Toy API.\"\"\"\n");
    });
    assert_eq!(extension_only, [""; 0]);
}

#[test]
fn allowlist_lines_are_well_formed_and_sorted() {
    let (lines, problems) = parse_allowlist("entries toy/repos.bzl\ntitle toy/lang.json\n");
    assert_eq!(lines.len(), 2);
    assert_eq!(problems, [""; 0]);
    let (_, problems) =
        parse_allowlist("title toy/lang.json\nentries toy/repos.bzl\nstyle toy\ntitle\n");
    assert_eq!(
        problems,
        [
            "not sorted or repeated: \"entries toy/repos.bzl\"",
            "malformed line: \"style toy\"",
            "malformed line: \"title\"",
        ]
    );
}

#[test]
fn a_violation_without_a_line_and_a_line_without_a_violation_both_fail() {
    let mut files = toy();
    put(&mut files, "toy/repos.bzl", "");
    let tree = Tree::new(files.keys().cloned().collect(), files);
    let found = lint(&tree);
    let allowed = |text: &str| parse_allowlist(text).0;
    assert_eq!(
        differences(&found, &allowed("entries toy/repos.bzl\n")),
        [""; 0]
    );
    assert_eq!(
        differences(&found, &allowed("")),
        ["entries toy/repos.bzl: not a skeleton entry; fix it"]
    );
    assert_eq!(
        differences(
            &found,
            &allowed("entries toy/repos.bzl\ntitle toy/lang.json\n")
        ),
        [format!(
            "title toy/lang.json: no such violation; delete the line from {ALLOWLIST}"
        )]
    );
}
