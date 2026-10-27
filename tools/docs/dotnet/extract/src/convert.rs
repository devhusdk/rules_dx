//! Maps the dotnet API facts onto the versioned documentation IR.

use crate::types::{parse_api, Api, Item, SCHEMA};
use documentation_ir::proto::{
    DocIr, Extension, Param, Relations, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::{escape_id_segment, SCHEMA_MAJOR, SCHEMA_MINOR};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// One declared source file of the documented library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub exec_path: String,
    pub file: String,
    pub generated: bool,
}

/// The identity inputs of one conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub language: String,
    pub package: String,
    pub config: String,
    pub sources: Vec<Source>,
}

/// Parses the tab-separated srcs manifest.
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
            exec_path: (*exec_path).to_owned(),
            file: (*file).to_owned(),
            generated,
        });
    }
    Ok(sources)
}

/// Converts one dx.dotnet-api/1 document into a validated IR shard.
pub fn convert(json: &str, settings: &Settings) -> Result<DocIr, String> {
    let api = parse_api(json)?;
    if api.language != settings.language {
        return Err(format!(
            "API JSON language '{}' does not match '{}'",
            api.language, settings.language
        ));
    }
    if api.items.is_empty() {
        return Err("API JSON has no public items".to_owned());
    }
    let ids = assign_ids(&api, settings)?;
    let mut symbols = Vec::with_capacity(api.items.len());
    for item in &api.items {
        symbols.push(symbol(&api, item, &ids, settings)?);
    }
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: settings.language.clone(),
        package: settings.package.clone(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

fn assign_ids(api: &Api, settings: &Settings) -> Result<BTreeMap<String, String>, String> {
    let by_doc_id: BTreeMap<&str, &Item> = {
        let mut map = BTreeMap::new();
        for item in &api.items {
            if map.insert(item.doc_id.as_str(), item).is_some() {
                return Err(format!("duplicate doc id '{}'", item.doc_id));
            }
        }
        map
    };
    let mut siblings: BTreeMap<(Option<&str>, &str), usize> = BTreeMap::new();
    for item in &api.items {
        *siblings
            .entry((item.parent_doc_id.as_deref(), item.name.as_str()))
            .or_default() += 1;
    }
    let mut ids = BTreeMap::new();
    let mut folded: BTreeMap<String, String> = BTreeMap::new();
    for item in &api.items {
        let mut segments = Vec::new();
        let mut current = item;
        for _ in 0..=api.items.len() {
            let overloaded = siblings
                .get(&(current.parent_doc_id.as_deref(), current.name.as_str()))
                .is_some_and(|count| *count > 1);
            segments.push(segment_group(current, overloaded)?);
            match current.parent_doc_id.as_deref() {
                None => break,
                Some(parent) => {
                    current = by_doc_id.get(parent).copied().ok_or_else(|| {
                        format!("'{}' has unknown parent '{parent}'", current.doc_id)
                    })?;
                }
            }
        }
        if current.parent_doc_id.is_some() {
            return Err(format!("parent cycle at '{}'", item.doc_id));
        }
        segments.reverse();
        let id = format!(
            "{}:{}:{}",
            settings.language,
            settings.package,
            segments.join(".")
        );
        if let Some(other) = folded.insert(id.to_ascii_lowercase(), item.doc_id.clone()) {
            return Err(format!(
                "id collision after case folding between '{other}' and '{}': {id}",
                item.doc_id
            ));
        }
        ids.insert(item.doc_id.clone(), id);
    }
    Ok(ids)
}

fn segment_group(item: &Item, overloaded: bool) -> Result<String, String> {
    if item.name.is_empty() {
        return Err(format!("'{}' has an empty name", item.doc_id));
    }
    if item.kind == "namespace" {
        return Ok(item
            .name
            .split('.')
            .map(escape_id_segment)
            .collect::<Vec<_>>()
            .join("."));
    }
    let mut segment = escape_id_segment(&item.name);
    if overloaded {
        let digest = Sha256::digest(item.doc_id.as_bytes());
        let hex: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        segment.push_str("~~");
        segment.push_str(&hex);
    }
    Ok(segment)
}

fn symbol_kind(kind: &str) -> Result<SymbolKind, String> {
    Ok(match kind {
        "class" | "struct" | "record" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "enum" | "union" => SymbolKind::Enum,
        "enum_field" | "union_case" => SymbolKind::EnumVariant,
        "method" | "constructor" | "operator" => SymbolKind::Method,
        "property" | "indexer" | "event" => SymbolKind::Property,
        "field" => SymbolKind::Field,
        "const" => SymbolKind::Constant,
        "delegate" | "abbreviation" => SymbolKind::TypeAlias,
        "namespace" | "module" => SymbolKind::Module,
        "function" => SymbolKind::Function,
        other => return Err(format!("unknown item kind '{other}'")),
    })
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.strip_prefix("./").unwrap_or(&path).to_owned()
}

fn source_ref(item: &Item, settings: &Settings) -> Result<Option<SourceRef>, String> {
    if item.file.is_empty() {
        return Ok(None);
    }
    let file = normalize(&item.file);
    let found = settings.sources.iter().find(|source| {
        let exec = normalize(&source.exec_path);
        file == exec || file.ends_with(&format!("/{exec}"))
    });
    let Some(source) = found else {
        return Err(format!(
            "'{}' is declared in '{}', which is not a declared source",
            item.doc_id, item.file
        ));
    };
    Ok(Some(SourceRef {
        file: source.file.clone(),
        line: item.line,
        generated: source.generated,
    }))
}

fn join_docs(item: &Item) -> String {
    [item.summary.trim(), item.remarks.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn symbol(
    api: &Api,
    item: &Item,
    ids: &BTreeMap<String, String>,
    settings: &Settings,
) -> Result<Symbol, String> {
    let lang = &settings.language;
    let id = ids
        .get(&item.doc_id)
        .cloned()
        .ok_or_else(|| format!("no id for '{}'", item.doc_id))?;
    let mut config = format!("assembly={};api={SCHEMA}", api.assembly);
    if !settings.config.is_empty() {
        config.push_str(&format!(";config={}", settings.config));
    }
    let mut missing: BTreeSet<String> = item.incomplete.iter().cloned().collect();
    if item.summary.trim().is_empty() {
        missing.insert("summary".to_owned());
    }
    let mut extensions = vec![Extension {
        key: format!("{lang}.config"),
        value: config.into_bytes(),
    }];
    extensions.push(Extension {
        key: format!("{lang}.incomplete"),
        value: missing
            .into_iter()
            .collect::<Vec<_>>()
            .join(";")
            .into_bytes(),
    });
    extensions.sort_by(|a, b| a.key.cmp(&b.key));
    let member_of = item
        .parent_doc_id
        .iter()
        .filter_map(|parent| ids.get(parent).cloned())
        .collect();
    let mut implements: Vec<String> = item
        .implements
        .iter()
        .filter_map(|target| ids.get(target).cloned())
        .collect();
    implements.sort();
    implements.dedup();
    let relations = Relations {
        member_of,
        implements,
        reexported_as: Vec::new(),
    };
    Ok(Symbol {
        id,
        kind: symbol_kind(&item.kind)? as i32,
        signature_text: item.signature.clone(),
        doc_markdown: join_docs(item),
        params: item
            .params
            .iter()
            .map(|param| Param {
                name: param.name.clone(),
                r#type: param.type_name.clone(),
                doc: param.doc.trim().to_owned(),
            })
            .collect(),
        returns: item.returns.as_ref().map(|returns| Returns {
            r#type: returns.type_name.clone(),
            doc: returns.doc.trim().to_owned(),
        }),
        examples: item.examples.clone(),
        source: source_ref(item, settings)?,
        visibility: Visibility::Public as i32,
        relations: Some(relations),
        extensions,
        display_name: item.display_name.clone(),
    })
}
