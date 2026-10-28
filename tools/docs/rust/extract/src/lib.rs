//! Converts nightly rustdoc JSON into one validated DocIr shard.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;

use documentation_ir::proto::{DocIr, Relations, SourceRef, Symbol, SymbolKind, Visibility};
use documentation_ir::{encode_shard, SCHEMA_MAJOR, SCHEMA_MINOR};
use rustdoc_types::{
    Crate, GenericArg, GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind, Generics,
    Id, Item, ItemEnum, MacroKind, Path, StructKind, Term, TraitBoundModifier, Type, VariantKind,
    WherePredicate, FORMAT_VERSION,
};

/// The inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: String,
    pub package: String,
    pub krate: String,
    pub srcs_file: String,
    pub out: String,
}

/// One crate source as the build sees it and as the docs show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub exec_path: String,
    pub file: String,
    pub generated: bool,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> &'static str {
    "usage: rust_extract --json FILE --package NAME --crate NAME --srcs-file FILE --out FILE"
}

/// Parses argv into inputs, or returns the rejection text.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        let slot = match flag {
            "--json" => &mut options.json,
            "--package" => &mut options.package,
            "--crate" => &mut options.krate,
            "--srcs-file" => &mut options.srcs_file,
            "--out" => &mut options.out,
            _ => return Err(format!("unknown argument '{flag}'")),
        };
        *slot = value.clone();
        index += 2;
    }
    for (flag, value) in [
        ("--json", &options.json),
        ("--package", &options.package),
        ("--crate", &options.krate),
        ("--srcs-file", &options.srcs_file),
        ("--out", &options.out),
    ] {
        if value.is_empty() {
            return Err(format!("{flag} is required"));
        }
    }
    Ok(options)
}

/// Parses the tab-separated srcs file into crate sources.
pub fn parse_sources(text: &str) -> Result<Vec<Source>, String> {
    let mut sources = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [exec_path, file, generated] = fields.as_slice() else {
            return Err(format!("srcs line needs three fields: {line}"));
        };
        let generated = match *generated {
            "1" => true,
            "0" => false,
            other => return Err(format!("srcs line has a bad generated flag '{other}'")),
        };
        sources.push(Source {
            exec_path: exec_path.to_string(),
            file: file.to_string(),
            generated,
        });
    }
    Ok(sources)
}

/// Maps one rustdoc span file to its docs path, or None when the crate does not own it.
pub fn map_source(sources: &[Source], span_file: &str) -> Option<(String, bool)> {
    let file = span_file.replace('\\', "/");
    let file = file.strip_prefix("./").unwrap_or(&file);
    for source in sources {
        if let Some(dir) = source.exec_path.strip_suffix('/') {
            let dir = format!("{dir}/");
            let at = if file.starts_with(&dir) {
                Some(0)
            } else {
                file.find(&format!("/{dir}")).map(|at| at + 1)
            };
            if let Some(at) = at {
                let rest = &file[at + dir.len()..];
                if !rest.is_empty() {
                    return Some((format!("{}{rest}", source.file), source.generated));
                }
            }
        } else if file == source.exec_path || file.ends_with(&format!("/{}", source.exec_path)) {
            return Some((source.file.clone(), source.generated));
        }
    }
    None
}

/// Reads, converts and writes one shard, returning "" on success.
pub fn run(options: &Options) -> String {
    let json = match fs::read_to_string(&options.json) {
        Ok(json) => json,
        Err(error) => return format!("cannot read rustdoc JSON '{}': {error}", options.json),
    };
    let srcs = match fs::read_to_string(&options.srcs_file) {
        Ok(srcs) => srcs,
        Err(error) => return format!("cannot read srcs file '{}': {error}", options.srcs_file),
    };
    let sources = match parse_sources(&srcs) {
        Ok(sources) => sources,
        Err(error) => return error,
    };
    let bytes = match convert(&json, &options.package, &options.krate, &sources) {
        Ok(bytes) => bytes,
        Err(error) => return error,
    };
    let partial = format!("{}.partial", options.out);
    if let Err(error) = fs::write(&partial, bytes) {
        let _ = fs::remove_file(&partial);
        return format!("cannot write '{}': {error}", options.out);
    }
    if let Err(error) = fs::rename(&partial, &options.out) {
        let _ = fs::remove_file(&partial);
        return format!("cannot write '{}': {error}", options.out);
    }
    String::new()
}

/// Converts one rustdoc JSON text into encoded shard bytes.
pub fn convert(
    json: &str,
    package: &str,
    krate: &str,
    sources: &[Source],
) -> Result<Vec<u8>, String> {
    encode_shard(&extract(json, package, krate, sources)?).map_err(|error| format!("{error:?}"))
}

/// Converts one rustdoc JSON text into a sorted shard.
pub fn extract(
    json: &str,
    package: &str,
    krate: &str,
    sources: &[Source],
) -> Result<DocIr, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("malformed rustdoc JSON: {error}"))?;
    let found = value
        .get("format_version")
        .and_then(serde_json::Value::as_u64);
    if found != Some(u64::from(FORMAT_VERSION)) {
        let found = found.map_or_else(|| "none".to_string(), |v| v.to_string());
        return Err(format!(
            "rustdoc JSON format_version {found} is not the supported {FORMAT_VERSION}"
        ));
    }
    let parsed: Crate = serde_json::from_value(value)
        .map_err(|error| format!("malformed rustdoc JSON: {error}"))?;
    let root = parsed
        .index
        .get(&parsed.root)
        .ok_or_else(|| "rustdoc JSON has no root module".to_string())?;
    if root.name.as_deref() != Some(krate) {
        return Err(format!(
            "rustdoc JSON documents '{}', not crate '{krate}'",
            root.name.as_deref().unwrap_or("")
        ));
    }
    let mut walker = Walker {
        krate: &parsed,
        prefix: format!("rust:{package}:"),
        sources,
        canonical: BTreeMap::new(),
        entries: Vec::new(),
        uses: VecDeque::new(),
        globs: VecDeque::new(),
        seen: BTreeSet::new(),
        aliases: Vec::new(),
    };
    walker.module(parsed.root, &[])?;
    walker.resolve_uses()?;
    let symbols = walker.finish()?;
    if symbols.is_empty() {
        return Err(format!("crate '{krate}' has no public API"));
    }
    Ok(DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "rust".to_string(),
        package: package.to_string(),
        symbols,
    })
}

type Pos = (String, u64, u64);

/// A rustdoc item reference whose final symbol id is known only after every
/// id has been assigned.
struct Ref {
    id: Id,
    fallback: String,
}

/// One symbol before ids are assigned.
struct Entry {
    base: String,
    rank: u8,
    tag: &'static str,
    /// Always carries its `@tag` suffix (glob re-exports of foreign modules).
    forced: bool,
    pos: Pos,
    symbol: Symbol,
    owner: Option<Id>,
    implements: Vec<Ref>,
    relations: bool,
}

/// One re-export name of an item that already has an entry.
struct AliasCand {
    base: String,
    rank: u8,
    tag: &'static str,
    pos: Pos,
    target: Id,
    glob: bool,
}

#[derive(Clone, Copy)]
enum Cand {
    Entry(usize),
    Alias(usize),
}

struct Walker<'a> {
    krate: &'a Crate,
    prefix: String,
    sources: &'a [Source],
    canonical: BTreeMap<Id, usize>,
    entries: Vec<Entry>,
    uses: VecDeque<(Id, Vec<String>, bool)>,
    globs: VecDeque<(Id, Vec<String>, bool)>,
    seen: BTreeSet<(Id, Vec<String>)>,
    aliases: Vec<AliasCand>,
}

impl<'a> Walker<'a> {
    fn item(&self, id: Id) -> Option<&'a Item> {
        self.krate.index.get(&id)
    }

    fn queue_use(&mut self, id: Id, path: Vec<String>, via_glob: bool) {
        let Some(ItemEnum::Use(import)) = self.item(id).map(|item| &item.inner) else {
            return;
        };
        if !self.seen.insert((id, path.clone())) {
            return;
        }
        if import.is_glob {
            self.globs.push_back((id, path, via_glob));
        } else {
            self.uses.push_back((id, path, via_glob));
        }
    }

    fn module(&mut self, id: Id, path: &[String]) -> Result<(), String> {
        let Some(ItemEnum::Module(module)) = self.item(id).map(|item| &item.inner) else {
            return Ok(());
        };
        for child in &module.items {
            let Some(item) = self.item(*child) else {
                continue;
            };
            if !is_public(item) {
                continue;
            }
            if matches!(item.inner, ItemEnum::Use(_)) {
                self.queue_use(*child, path.to_vec(), false);
            } else if let Some(name) = item.name.clone() {
                self.define(item, path, &name)?;
            }
        }
        Ok(())
    }

    fn define(&mut self, item: &'a Item, path: &[String], name: &str) -> Result<(), String> {
        if self.canonical.contains_key(&item.id) {
            return Ok(());
        }
        let Some(kind) = kind_of(&item.inner) else {
            return Ok(());
        };
        let segment = match &item.inner {
            ItemEnum::Macro(_) => format!("{name}!"),
            ItemEnum::ProcMacro(proc_macro) if proc_macro.kind == MacroKind::Bang => {
                format!("{name}!")
            }
            _ => name.to_string(),
        };
        let mut qualified = path.to_vec();
        qualified.push(segment);
        let (rank, tag) = item_namespace(&item.inner);
        let base = self.id(&qualified);
        let signature = signature(item, name);
        let idx = self.push(item, base, (rank, tag), kind, signature, None)?;
        self.canonical.insert(item.id, idx);
        match &item.inner {
            ItemEnum::Module(_) => self.module(item.id, &qualified)?,
            ItemEnum::Struct(data) => {
                if let StructKind::Plain { fields, .. } = &data.kind {
                    self.fields(fields, &qualified, item.id)?;
                }
                self.impls(&data.impls, &qualified, item.id)?;
            }
            ItemEnum::Union(data) => {
                self.fields(&data.fields, &qualified, item.id)?;
                self.impls(&data.impls, &qualified, item.id)?;
            }
            ItemEnum::Enum(data) => {
                for variant in &data.variants {
                    if let Some(variant) = self.item(*variant) {
                        self.member(variant, &qualified, Some(item.id), SymbolKind::EnumVariant)?;
                    }
                }
                self.impls(&data.impls, &qualified, item.id)?;
            }
            ItemEnum::Trait(data) => {
                for child in &data.items {
                    if let Some(child) = self.item(*child) {
                        if let Some(kind) = member_kind(&child.inner) {
                            self.member(child, &qualified, Some(item.id), kind)?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn fields(&mut self, fields: &[Id], path: &[String], owner: Id) -> Result<(), String> {
        for field in fields {
            if let Some(field) = self.item(*field).filter(|field| is_public(field)) {
                self.member(field, path, Some(owner), SymbolKind::Field)?;
            }
        }
        Ok(())
    }

    fn impls(&mut self, impls: &[Id], path: &[String], owner: Id) -> Result<(), String> {
        let mut implements = Vec::new();
        for id in impls {
            let Some(ItemEnum::Impl(block)) = self.item(*id).map(|item| &item.inner) else {
                continue;
            };
            if block.is_synthetic || block.blanket_impl.is_some() || block.is_negative {
                continue;
            }
            if let Some(trait_) = &block.trait_ {
                implements.push(self.path_ref(trait_));
                continue;
            }
            for child in &block.items {
                let Some(child) = self.item(*child).filter(|child| is_public(child)) else {
                    continue;
                };
                if let Some(kind) = member_kind(&child.inner) {
                    self.member(child, path, Some(owner), kind)?;
                }
            }
        }
        if let Some(&idx) = self.canonical.get(&owner) {
            self.entries[idx].implements = implements;
            self.entries[idx].relations = true;
        }
        Ok(())
    }

    fn member(
        &mut self,
        item: &'a Item,
        path: &[String],
        owner: Option<Id>,
        kind: SymbolKind,
    ) -> Result<(), String> {
        let Some(name) = item.name.as_deref() else {
            return Ok(());
        };
        if self.canonical.contains_key(&item.id) {
            return Ok(());
        }
        let mut qualified = path.to_vec();
        qualified.push(name.to_string());
        let base = self.id(&qualified);
        let namespace = member_namespace(kind);
        let idx = self.push(item, base, namespace, kind, signature(item, name), owner)?;
        self.canonical.insert(item.id, idx);
        Ok(())
    }

    fn push(
        &mut self,
        item: &Item,
        base: String,
        (rank, tag): (u8, &'static str),
        kind: SymbolKind,
        signature_text: String,
        owner: Option<Id>,
    ) -> Result<usize, String> {
        let source = self.source(item)?;
        let symbol = Symbol {
            kind: kind as i32,
            signature_text,
            doc_markdown: item.docs.clone().unwrap_or_default(),
            source,
            visibility: Visibility::Public as i32,
            ..Symbol::default()
        };
        self.entries.push(Entry {
            base,
            rank,
            tag,
            forced: false,
            pos: self.pos(item),
            symbol,
            owner,
            implements: Vec::new(),
            relations: false,
        });
        Ok(self.entries.len() - 1)
    }

    fn pos(&self, item: &Item) -> Pos {
        match item.span.as_ref() {
            Some(span) => {
                let file = span.filename.to_string_lossy();
                let file = map_source(self.sources, &file)
                    .map_or_else(|| file.to_string(), |(mapped, _)| mapped);
                (file, span.begin.0 as u64, span.begin.1 as u64)
            }
            None => (String::new(), 0, 0),
        }
    }

    fn source(&self, item: &Item) -> Result<Option<SourceRef>, String> {
        if item.crate_id != 0 {
            return Ok(None);
        }
        let Some(span) = item.span.as_ref() else {
            return Ok(None);
        };
        let span_file = span.filename.to_string_lossy();
        let (file, generated) = map_source(self.sources, &span_file).ok_or_else(|| {
            format!(
                "source '{span_file}' of '{}' is not a crate source",
                item_label(item)
            )
        })?;
        Ok(Some(SourceRef {
            file,
            line: span.begin.0 as u64,
            generated,
        }))
    }

    fn resolve_uses(&mut self) -> Result<(), String> {
        // Explicit `pub use` items resolve before any glob, as in rustc.
        while let Some((use_id, path, via_glob)) =
            self.uses.pop_front().or_else(|| self.globs.pop_front())
        {
            let Some(item) = self.item(use_id) else {
                continue;
            };
            let ItemEnum::Use(import) = &item.inner else {
                continue;
            };
            let target = import.id.and_then(|id| self.item(id));
            if import.is_glob {
                let local = target.filter(|target| target.crate_id == 0);
                match local.map(|target| &target.inner) {
                    Some(ItemEnum::Module(module)) => {
                        for child in &module.items {
                            let Some(child) = self.item(*child).filter(|child| is_public(child))
                            else {
                                continue;
                            };
                            if matches!(child.inner, ItemEnum::Use(_)) {
                                self.queue_use(child.id, path.clone(), true);
                            } else if let Some(name) = child.name.clone() {
                                self.alias_or_define(child, &path, &name, item, true)?;
                            }
                        }
                    }
                    Some(ItemEnum::Enum(data)) => {
                        for variant in &data.variants {
                            let Some(variant) = self.item(*variant) else {
                                continue;
                            };
                            if let Some(name) = variant.name.clone() {
                                self.alias_or_define(variant, &path, &name, item, true)?;
                            }
                        }
                    }
                    _ => self.glob_symbol(item, import, &path)?,
                }
                continue;
            }
            match target {
                Some(target) if target.crate_id == 0 => {
                    self.alias_or_define(target, &path, &import.name, item, via_glob)?;
                }
                _ => {
                    let mut qualified = path.clone();
                    qualified.push(import.name.clone());
                    let base = self.id(&qualified);
                    let last = import.source.rsplit("::").next().unwrap_or("");
                    let signature = if last == import.name {
                        format!("pub use {};", import.source)
                    } else {
                        format!("pub use {} as {};", import.source, import.name)
                    };
                    let summary = import.id.and_then(|id| self.krate.paths.get(&id));
                    let kind = summary.map_or(SymbolKind::Unspecified, |summary| {
                        summary_kind(&summary.kind)
                    });
                    let namespace =
                        summary.map_or((3, "item"), |summary| summary_namespace(&summary.kind));
                    self.push(item, base, namespace, kind, signature, None)?;
                }
            }
        }
        Ok(())
    }

    /// A glob of a foreign or unknown module cannot be expanded, so it
    /// becomes one symbol that always carries `@glob`.
    fn glob_symbol(
        &mut self,
        item: &'a Item,
        import: &rustdoc_types::Use,
        path: &[String],
    ) -> Result<(), String> {
        let source = import.source.trim_end_matches("::*");
        let last = source.rsplit("::").next().unwrap_or("");
        let last = if last.is_empty() { "glob" } else { last };
        let mut qualified = path.to_vec();
        qualified.push(last.to_string());
        let base = self.id(&qualified);
        let idx = self.push(
            item,
            base,
            (3, "glob"),
            SymbolKind::Unspecified,
            format!("pub use {source}::*;"),
            None,
        )?;
        self.entries[idx].forced = true;
        Ok(())
    }

    fn alias_or_define(
        &mut self,
        target: &'a Item,
        path: &[String],
        name: &str,
        use_item: &Item,
        glob: bool,
    ) -> Result<(), String> {
        if let Some(&idx) = self.canonical.get(&target.id) {
            let canonical = &self.entries[idx];
            let bang = if canonical.base.ends_with('!') {
                "!"
            } else {
                ""
            };
            let mut qualified = path.to_vec();
            qualified.push(format!("{name}{bang}"));
            let base = self.id(&qualified);
            if base != canonical.base {
                let alias = AliasCand {
                    base,
                    rank: canonical.rank,
                    tag: canonical.tag,
                    pos: self.pos(use_item),
                    target: target.id,
                    glob,
                };
                self.aliases.push(alias);
            }
            return Ok(());
        }
        if matches!(target.inner, ItemEnum::Variant(_)) {
            return self.member(target, path, None, SymbolKind::EnumVariant);
        }
        self.define(target, path, name)
    }

    fn id(&self, qualified: &[String]) -> String {
        format!("{}{}", self.prefix, qualified.join("."))
    }

    fn path_ref(&self, path: &Path) -> Ref {
        let fallback = match self.krate.paths.get(&path.id) {
            Some(summary) if summary.crate_id == 0 && summary.path.len() > 1 => {
                format!("{}{}", self.prefix, summary.path[1..].join("."))
            }
            Some(summary) if summary.path.len() > 1 => {
                format!("rust:{}:{}", summary.path[0], summary.path[1..].join("."))
            }
            _ => format!("rust::{}", path.path.replace("::", ".")),
        };
        Ref {
            id: path.id,
            fallback,
        }
    }

    /// Assigns the final ids once the whole crate is collected. Only items
    /// that share a base id get an `@tag` suffix, so ids never depend on the
    /// order rustdoc listed the items in.
    fn finish(self) -> Result<Vec<Symbol>, String> {
        let Walker {
            entries,
            aliases,
            canonical,
            ..
        } = self;
        // One alias per (name, target); an explicit `pub use` beats a glob.
        let mut kept: BTreeMap<(&str, Id), usize> = BTreeMap::new();
        for (index, alias) in aliases.iter().enumerate() {
            match kept.entry((alias.base.as_str(), alias.target)) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(index);
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    if aliases[*slot.get()].glob && !alias.glob {
                        slot.insert(index);
                    }
                }
            }
        }
        let mut groups: BTreeMap<&str, Vec<Cand>> = BTreeMap::new();
        for (index, entry) in entries.iter().enumerate() {
            groups
                .entry(entry.base.as_str())
                .or_default()
                .push(Cand::Entry(index));
        }
        for index in kept.into_values() {
            groups
                .entry(aliases[index].base.as_str())
                .or_default()
                .push(Cand::Alias(index));
        }
        let rank_of = |cand: &Cand| match cand {
            Cand::Entry(i) => entries[*i].rank,
            Cand::Alias(i) => aliases[*i].rank,
        };
        let key_of = |cand: &Cand| match cand {
            Cand::Entry(i) => (entries[*i].rank, entries[*i].pos.clone(), entries[*i].tag),
            Cand::Alias(i) => (aliases[*i].rank, aliases[*i].pos.clone(), aliases[*i].tag),
        };
        let mut final_entry = vec![String::new(); entries.len()];
        let mut final_alias: Vec<Option<String>> = vec![None; aliases.len()];
        for (base, cands) in groups {
            let mut forced = Vec::new();
            let mut live = Vec::new();
            for cand in &cands {
                match cand {
                    Cand::Entry(i) if entries[*i].forced => forced.push(*cand),
                    _ => live.push(*cand),
                }
            }
            // A glob name is dropped when another item has the same name in
            // the same namespace; an explicit name is an error instead.
            let mut survivors = Vec::new();
            for cand in &live {
                if let Cand::Alias(i) = cand {
                    let rivals: Vec<&Cand> = live
                        .iter()
                        .filter(|other| {
                            !matches!((cand, other), (Cand::Alias(a), Cand::Alias(b)) if a == b)
                                && rank_of(other) == rank_of(cand)
                        })
                        .collect();
                    if aliases[*i].glob {
                        if !rivals.is_empty() {
                            continue;
                        }
                    } else if let Some(rival) = rivals
                        .iter()
                        .find(|r| !matches!(r, Cand::Alias(j) if aliases[*j].glob))
                    {
                        let other = match rival {
                            Cand::Entry(j) => entries[*j].symbol.signature_text.clone(),
                            Cand::Alias(j) => format!("a re-export of {}", aliases[*j].base),
                        };
                        return Err(format!(
                            "re-export name '{base}' collides with '{other}' in the same namespace"
                        ));
                    }
                }
                survivors.push(*cand);
            }
            survivors.sort_by_key(key_of);
            forced.sort_by_key(key_of);
            for (group, plain_first) in [(&survivors, true), (&forced, false)] {
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for (n, cand) in group.iter().enumerate() {
                    let tag = key_of(cand).2;
                    let id = if plain_first && n == 0 {
                        base.to_string()
                    } else {
                        let count = counts.entry(tag).or_insert(0);
                        *count += 1;
                        if *count == 1 {
                            format!("{base}@{tag}")
                        } else {
                            format!("{base}@{tag}-{count}")
                        }
                    };
                    match cand {
                        Cand::Entry(i) => final_entry[*i] = id,
                        Cand::Alias(i) => final_alias[*i] = Some(id),
                    }
                }
            }
        }
        let resolve = |id: &Id| canonical.get(id).map(|index| final_entry[*index].clone());
        let mut reexported: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
        for (index, alias) in aliases.iter().enumerate() {
            if let (Some(id), Some(&target)) = (&final_alias[index], canonical.get(&alias.target)) {
                reexported.entry(target).or_default().insert(id.clone());
            }
        }
        let mut symbols = Vec::with_capacity(entries.len());
        for (index, entry) in entries.into_iter().enumerate() {
            let mut symbol = entry.symbol;
            symbol.id = final_entry[index].clone();
            let member_of: Vec<String> = entry.owner.iter().filter_map(&resolve).collect();
            let implements: BTreeSet<String> = entry
                .implements
                .iter()
                .map(|r| resolve(&r.id).unwrap_or_else(|| r.fallback.clone()))
                .collect();
            let reexported_as: Vec<String> = reexported
                .remove(&index)
                .map(|set| set.into_iter().collect())
                .unwrap_or_default();
            if entry.relations
                || !member_of.is_empty()
                || !implements.is_empty()
                || !reexported_as.is_empty()
            {
                symbol.relations = Some(Relations {
                    member_of,
                    implements: implements.into_iter().collect(),
                    reexported_as,
                });
            }
            symbols.push(symbol);
        }
        symbols.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(symbols)
    }
}

fn is_public(item: &Item) -> bool {
    item.visibility == rustdoc_types::Visibility::Public
}

fn item_label(item: &Item) -> String {
    item.name
        .clone()
        .unwrap_or_else(|| format!("item {}", item.id.0))
}

fn kind_of(inner: &ItemEnum) -> Option<SymbolKind> {
    Some(match inner {
        ItemEnum::Module(module) if module.is_stripped => return None,
        ItemEnum::Module(_) => SymbolKind::Module,
        ItemEnum::Struct(_) | ItemEnum::Union(_) | ItemEnum::ExternType => SymbolKind::Class,
        ItemEnum::Enum(_) => SymbolKind::Enum,
        ItemEnum::Function(_) => SymbolKind::Function,
        ItemEnum::Trait(_) | ItemEnum::TraitAlias(_) => SymbolKind::Interface,
        ItemEnum::TypeAlias(_) => SymbolKind::TypeAlias,
        ItemEnum::Constant { .. } | ItemEnum::Static(_) => SymbolKind::Constant,
        ItemEnum::Macro(_) | ItemEnum::ProcMacro(_) => SymbolKind::Macro,
        _ => return None,
    })
}

fn member_kind(inner: &ItemEnum) -> Option<SymbolKind> {
    Some(match inner {
        ItemEnum::Function(_) => SymbolKind::Method,
        ItemEnum::AssocConst { .. } => SymbolKind::Constant,
        ItemEnum::AssocType { .. } => SymbolKind::TypeAlias,
        _ => return None,
    })
}

/// Rust keeps types, values and macros in separate namespaces; the rank orders
/// colliding ids and the tag names the loser.
fn item_namespace(inner: &ItemEnum) -> (u8, &'static str) {
    match inner {
        ItemEnum::Module(_) => (0, "mod"),
        ItemEnum::Struct(_) => (0, "struct"),
        ItemEnum::Union(_) => (0, "union"),
        ItemEnum::Enum(_) => (0, "enum"),
        ItemEnum::Trait(_) | ItemEnum::TraitAlias(_) => (0, "trait"),
        ItemEnum::TypeAlias(_) | ItemEnum::ExternType => (0, "type"),
        ItemEnum::Function(_) => (1, "fn"),
        ItemEnum::Constant { .. } => (1, "const"),
        ItemEnum::Static(_) => (1, "static"),
        ItemEnum::Macro(_) => (2, "macro"),
        ItemEnum::ProcMacro(proc_macro) => match proc_macro.kind {
            MacroKind::Bang => (2, "macro"),
            MacroKind::Attr => (2, "attr"),
            MacroKind::Derive => (2, "derive"),
        },
        _ => (3, "item"),
    }
}

fn member_namespace(kind: SymbolKind) -> (u8, &'static str) {
    match kind {
        SymbolKind::EnumVariant => (0, "variant"),
        SymbolKind::Field => (0, "field"),
        SymbolKind::TypeAlias => (1, "type"),
        SymbolKind::Constant => (2, "const"),
        _ => (2, "method"),
    }
}

fn summary_namespace(kind: &rustdoc_types::ItemKind) -> (u8, &'static str) {
    use rustdoc_types::ItemKind as K;
    match kind {
        K::Module => (0, "mod"),
        K::Struct => (0, "struct"),
        K::Union => (0, "union"),
        K::Enum => (0, "enum"),
        K::Trait | K::TraitAlias => (0, "trait"),
        K::TypeAlias | K::ExternType => (0, "type"),
        K::Function => (1, "fn"),
        K::Constant => (1, "const"),
        K::Static => (1, "static"),
        K::Macro => (2, "macro"),
        K::ProcAttribute => (2, "attr"),
        K::ProcDerive => (2, "derive"),
        _ => (3, "item"),
    }
}

fn summary_kind(kind: &rustdoc_types::ItemKind) -> SymbolKind {
    use rustdoc_types::ItemKind as K;
    match kind {
        K::Module => SymbolKind::Module,
        K::Struct | K::Union | K::ExternType => SymbolKind::Class,
        K::Enum => SymbolKind::Enum,
        K::Variant => SymbolKind::EnumVariant,
        K::Function => SymbolKind::Function,
        K::Trait | K::TraitAlias => SymbolKind::Interface,
        K::TypeAlias | K::AssocType => SymbolKind::TypeAlias,
        K::Constant | K::Static | K::AssocConst => SymbolKind::Constant,
        K::Macro | K::ProcAttribute | K::ProcDerive => SymbolKind::Macro,
        _ => SymbolKind::Unspecified,
    }
}

/// Returns the declaration text shown for one item.
pub fn signature(item: &Item, name: &str) -> String {
    let vis = if is_public(item) { "pub " } else { "" };
    match &item.inner {
        ItemEnum::Module(_) => format!("{vis}mod {name}"),
        ItemEnum::Function(function) => {
            let header = &function.header;
            let mut text = String::from(vis);
            for (on, word) in [
                (header.is_const, "const "),
                (header.is_async, "async "),
                (header.is_unsafe, "unsafe "),
            ] {
                if on {
                    text.push_str(word);
                }
            }
            if let Some(abi) = abi(&header.abi) {
                text.push_str(&format!("extern \"{abi}\" "));
            }
            let inputs: Vec<String> = function
                .sig
                .inputs
                .iter()
                .map(|(arg, ty)| input(arg, ty))
                .chain(function.sig.is_c_variadic.then(|| "...".to_string()))
                .collect();
            text.push_str(&format!(
                "fn {name}{}({})",
                params(&function.generics.params),
                inputs.join(", ")
            ));
            if let Some(output) = &function.sig.output {
                text.push_str(&format!(" -> {}", ty(output)));
            }
            text + &where_clause(&function.generics)
        }
        ItemEnum::Struct(data) => {
            let head = format!("{vis}struct {name}{}", params(&data.generics.params));
            let clause = where_clause(&data.generics);
            match &data.kind {
                StructKind::Unit => format!("{head}{clause};"),
                StructKind::Tuple(fields) => {
                    format!("{head}({}){clause};", tuple_fields(item_fields(fields)))
                }
                StructKind::Plain { .. } => format!("{head}{clause} {{ .. }}"),
            }
        }
        ItemEnum::Union(data) => format!(
            "{vis}union {name}{}{} {{ .. }}",
            params(&data.generics.params),
            where_clause(&data.generics)
        ),
        ItemEnum::Enum(data) => format!(
            "{vis}enum {name}{}{} {{ .. }}",
            params(&data.generics.params),
            where_clause(&data.generics)
        ),
        ItemEnum::Variant(variant) => {
            let body = match &variant.kind {
                VariantKind::Plain => String::new(),
                VariantKind::Tuple(fields) => format!("({})", tuple_fields(item_fields(fields))),
                VariantKind::Struct { .. } => " { .. }".to_string(),
            };
            let discriminant = variant
                .discriminant
                .as_ref()
                .map(|d| format!(" = {}", d.expr))
                .unwrap_or_default();
            format!("{name}{body}{discriminant}")
        }
        ItemEnum::StructField(field) => format!("{vis}{name}: {}", ty(field)),
        ItemEnum::Trait(data) => {
            let mut text = String::from(vis);
            if data.is_unsafe {
                text.push_str("unsafe ");
            }
            if data.is_auto {
                text.push_str("auto ");
            }
            text.push_str(&format!("trait {name}{}", params(&data.generics.params)));
            if !data.bounds.is_empty() {
                text.push_str(&format!(": {}", bounds(&data.bounds)));
            }
            text + &where_clause(&data.generics)
        }
        ItemEnum::TraitAlias(data) => format!(
            "{vis}trait {name}{} = {}{};",
            params(&data.generics.params),
            bounds(&data.params),
            where_clause(&data.generics)
        ),
        ItemEnum::TypeAlias(alias) => format!(
            "{vis}type {name}{}{} = {};",
            params(&alias.generics.params),
            where_clause(&alias.generics),
            ty(&alias.type_)
        ),
        ItemEnum::Constant { type_, const_ } => {
            format!("{vis}const {name}: {} = {};", ty(type_), const_.expr)
        }
        ItemEnum::Static(data) => format!(
            "{vis}{}static {}{name}: {};",
            if data.is_unsafe { "unsafe " } else { "" },
            if data.is_mutable { "mut " } else { "" },
            ty(&data.type_)
        ),
        ItemEnum::Macro(text) => text.clone(),
        ItemEnum::ProcMacro(proc_macro) => match proc_macro.kind {
            MacroKind::Bang => format!("#[proc_macro] {name}!"),
            MacroKind::Attr => format!("#[proc_macro_attribute] {name}"),
            MacroKind::Derive => format!("#[proc_macro_derive({name})]"),
        },
        ItemEnum::ExternType => format!("{vis}type {name};"),
        ItemEnum::AssocConst { type_, value, .. } => match value {
            Some(value) => format!("{vis}const {name}: {} = {value};", ty(type_)),
            None => format!("{vis}const {name}: {};", ty(type_)),
        },
        ItemEnum::AssocType {
            generics,
            bounds: list,
            type_,
            ..
        } => {
            let mut text = format!("{vis}type {name}{}", params(&generics.params));
            if !list.is_empty() {
                text.push_str(&format!(": {}", bounds(list)));
            }
            text.push_str(&where_clause(generics));
            if let Some(type_) = type_ {
                text.push_str(&format!(" = {}", ty(type_)));
            }
            text + ";"
        }
        _ => name.to_string(),
    }
}

fn item_fields(fields: &[Option<Id>]) -> Vec<bool> {
    fields.iter().map(Option::is_some).collect()
}

fn tuple_fields(visible: Vec<bool>) -> String {
    visible
        .into_iter()
        .map(|shown| if shown { ".." } else { "_" })
        .collect::<Vec<_>>()
        .join(", ")
}

fn abi(abi: &rustdoc_types::Abi) -> Option<String> {
    use rustdoc_types::Abi as A;
    let (name, unwind) = match abi {
        A::Rust => return None,
        A::C { unwind } => ("C", *unwind),
        A::Cdecl { unwind } => ("cdecl", *unwind),
        A::Stdcall { unwind } => ("stdcall", *unwind),
        A::Fastcall { unwind } => ("fastcall", *unwind),
        A::Aapcs { unwind } => ("aapcs", *unwind),
        A::Win64 { unwind } => ("win64", *unwind),
        A::SysV64 { unwind } => ("sysv64", *unwind),
        A::System { unwind } => ("system", *unwind),
        A::Other(name) => return Some(name.clone()),
    };
    Some(if unwind {
        format!("{name}-unwind")
    } else {
        name.to_string()
    })
}

fn input(name: &str, type_: &Type) -> String {
    if name == "self" {
        match type_ {
            Type::Generic(generic) if generic == "Self" => return "self".to_string(),
            Type::BorrowedRef {
                lifetime,
                is_mutable,
                type_: inner,
            } if matches!(inner.as_ref(), Type::Generic(g) if g == "Self") => {
                let lifetime = lifetime
                    .as_ref()
                    .map(|l| format!("{l} "))
                    .unwrap_or_default();
                let mutable = if *is_mutable { "mut " } else { "" };
                return format!("&{lifetime}{mutable}self");
            }
            _ => {}
        }
    }
    format!("{name}: {}", ty(type_))
}

fn params(list: &[GenericParamDef]) -> String {
    let shown: Vec<String> = list
        .iter()
        .filter(|param| {
            !matches!(
                param.kind,
                GenericParamDefKind::Type {
                    is_synthetic: true,
                    ..
                }
            )
        })
        .map(param)
        .collect();
    if shown.is_empty() {
        String::new()
    } else {
        format!("<{}>", shown.join(", "))
    }
}

fn param(param: &GenericParamDef) -> String {
    match &param.kind {
        GenericParamDefKind::Lifetime { outlives } if outlives.is_empty() => param.name.clone(),
        GenericParamDefKind::Lifetime { outlives } => {
            format!("{}: {}", param.name, outlives.join(" + "))
        }
        GenericParamDefKind::Type {
            bounds: list,
            default,
            ..
        } => {
            let mut text = param.name.clone();
            if !list.is_empty() {
                text.push_str(&format!(": {}", bounds(list)));
            }
            if let Some(default) = default {
                text.push_str(&format!(" = {}", ty(default)));
            }
            text
        }
        GenericParamDefKind::Const { type_, default } => {
            let mut text = format!("const {}: {}", param.name, ty(type_));
            if let Some(default) = default {
                text.push_str(&format!(" = {default}"));
            }
            text
        }
    }
}

fn where_clause(generics: &Generics) -> String {
    if generics.where_predicates.is_empty() {
        return String::new();
    }
    let predicates: Vec<String> = generics
        .where_predicates
        .iter()
        .map(|predicate| match predicate {
            WherePredicate::BoundPredicate {
                type_,
                bounds: list,
                generic_params,
            } => format!("{}{}: {}", hrtb(generic_params), ty(type_), bounds(list)),
            WherePredicate::LifetimePredicate { lifetime, outlives } => {
                format!("{lifetime}: {}", outlives.join(" + "))
            }
            WherePredicate::EqPredicate { lhs, rhs } => format!("{} = {}", ty(lhs), term(rhs)),
        })
        .collect();
    format!(" where {}", predicates.join(", "))
}

fn hrtb(list: &[GenericParamDef]) -> String {
    if list.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = list.iter().map(param).collect();
        format!("for<{}> ", names.join(", "))
    }
}

fn bounds(list: &[GenericBound]) -> String {
    list.iter()
        .map(|bound| match bound {
            GenericBound::TraitBound {
                trait_,
                generic_params,
                modifier,
            } => {
                let modifier = match modifier {
                    TraitBoundModifier::None => "",
                    TraitBoundModifier::Maybe => "?",
                    TraitBoundModifier::MaybeConst => "~const ",
                };
                format!("{}{modifier}{}", hrtb(generic_params), path(trait_))
            }
            GenericBound::Outlives(lifetime) => lifetime.clone(),
            GenericBound::Use(args) => {
                let names: Vec<String> = args
                    .iter()
                    .map(|arg| match arg {
                        rustdoc_types::PreciseCapturingArg::Lifetime(name)
                        | rustdoc_types::PreciseCapturingArg::Param(name) => name.clone(),
                    })
                    .collect();
                format!("use<{}>", names.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn path(path: &Path) -> String {
    match path.args.as_deref() {
        Some(args) => format!("{}{}", path.path, generic_args(args)),
        None => path.path.clone(),
    }
}

fn generic_args(args: &GenericArgs) -> String {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            let mut parts: Vec<String> = args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Lifetime(lifetime) => lifetime.clone(),
                    GenericArg::Type(type_) => ty(type_),
                    GenericArg::Const(constant) => constant.expr.clone(),
                    GenericArg::Infer => "_".to_string(),
                })
                .collect();
            for constraint in constraints {
                let args = constraint
                    .args
                    .as_deref()
                    .map(generic_args)
                    .unwrap_or_default();
                parts.push(match &constraint.binding {
                    rustdoc_types::AssocItemConstraintKind::Equality(value) => {
                        format!("{}{args} = {}", constraint.name, term(value))
                    }
                    rustdoc_types::AssocItemConstraintKind::Constraint(list) => {
                        format!("{}{args}: {}", constraint.name, bounds(list))
                    }
                });
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!("<{}>", parts.join(", "))
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let inputs: Vec<String> = inputs.iter().map(ty).collect();
            match output {
                Some(output) => format!("({}) -> {}", inputs.join(", "), ty(output)),
                None => format!("({})", inputs.join(", ")),
            }
        }
        GenericArgs::ReturnTypeNotation => "(..)".to_string(),
    }
}

fn term(value: &Term) -> String {
    match value {
        Term::Type(type_) => ty(type_),
        Term::Constant(constant) => constant.expr.clone(),
    }
}

/// Returns the source text of one type.
pub fn ty(type_: &Type) -> String {
    match type_ {
        Type::ResolvedPath(resolved) => path(resolved),
        Type::DynTrait(dyn_trait) => {
            let mut parts: Vec<String> = dyn_trait
                .traits
                .iter()
                .map(|poly| format!("{}{}", hrtb(&poly.generic_params), path(&poly.trait_)))
                .collect();
            if let Some(lifetime) = &dyn_trait.lifetime {
                parts.push(lifetime.clone());
            }
            format!("dyn {}", parts.join(" + "))
        }
        Type::Generic(name) | Type::Primitive(name) => name.clone(),
        Type::FunctionPointer(pointer) => {
            let mut text = hrtb(&pointer.generic_params);
            if pointer.header.is_unsafe {
                text.push_str("unsafe ");
            }
            if let Some(abi) = abi(&pointer.header.abi) {
                text.push_str(&format!("extern \"{abi}\" "));
            }
            let inputs: Vec<String> = pointer
                .sig
                .inputs
                .iter()
                .map(|(name, type_)| {
                    if name.is_empty() || name == "_" {
                        ty(type_)
                    } else {
                        format!("{name}: {}", ty(type_))
                    }
                })
                .collect();
            text.push_str(&format!("fn({})", inputs.join(", ")));
            if let Some(output) = &pointer.sig.output {
                text.push_str(&format!(" -> {}", ty(output)));
            }
            text
        }
        Type::Tuple(items) if items.len() == 1 => format!("({},)", ty(&items[0])),
        Type::Tuple(items) => {
            format!("({})", items.iter().map(ty).collect::<Vec<_>>().join(", "))
        }
        Type::Slice(inner) => format!("[{}]", ty(inner)),
        Type::Array { type_, len } => format!("[{}; {len}]", ty(type_)),
        Type::Pat { type_, .. } => ty(type_),
        Type::ImplTrait(list) => format!("impl {}", bounds(list)),
        Type::Infer => "_".to_string(),
        Type::RawPointer { is_mutable, type_ } => {
            format!(
                "*{} {}",
                if *is_mutable { "mut" } else { "const" },
                ty(type_)
            )
        }
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            let lifetime = lifetime
                .as_ref()
                .map(|l| format!("{l} "))
                .unwrap_or_default();
            let mutable = if *is_mutable { "mut " } else { "" };
            format!("&{lifetime}{mutable}{}", ty(type_))
        }
        Type::QualifiedPath {
            name,
            args,
            self_type,
            trait_,
        } => {
            let args = args.as_deref().map(generic_args).unwrap_or_default();
            match trait_ {
                Some(trait_) => format!("<{} as {}>::{name}{args}", ty(self_type), path(trait_)),
                None => format!("{}::{name}{args}", ty(self_type)),
            }
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
