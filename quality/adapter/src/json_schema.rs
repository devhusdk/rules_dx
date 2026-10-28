//! Offline selection, gating and positioning for JSON Schema validation.
//!
//! `check-jsonschema` runs once per (schema, filetype) group. Everything that decides what
//! it may read lives here: which files a schema covers, which drafts are supported, and
//! which `$ref` targets have a checked-in copy. Findings are placed on source lines with
//! the shared pointer index.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::pointer_index::{self, escape, Index, SyntaxAt};

/// The `schema_version` of the config this module reads.
pub const CONFIG_VERSION: u32 = 1;

/// The `$schema` spellings dx accepts, without a trailing `#`. A missing `$schema` means 2020-12.
pub const DRAFTS: [&str; 5] = [
    "http://json-schema.org/draft-04/schema",
    "http://json-schema.org/draft-06/schema",
    "http://json-schema.org/draft-07/schema",
    "https://json-schema.org/draft/2019-09/schema",
    "https://json-schema.org/draft/2020-12/schema",
];

/// The file formats `check-jsonschema` is told to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Filetype {
    Json,
    Yaml,
    Toml,
}

impl Filetype {
    /// The value of `--force-filetype`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }

    /// The filetype a workspace path is read as, from its extension.
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
        match ext.as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema_version: u32,
    schemas: Vec<RawSchema>,
    #[serde(default)]
    resources: Vec<RawResource>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSchema {
    schema: String,
    #[serde(default)]
    files: Vec<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResource {
    schema: String,
    url: Option<String>,
}

/// One schema and the files it validates.
#[derive(Debug, Clone)]
pub struct Schema {
    pub path: PathBuf,
    pub url: Option<String>,
    pub globs: Vec<String>,
    matcher: GlobSet,
}

/// One staged schema document, with the URL it stands in for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub path: PathBuf,
    pub url: Option<String>,
}

/// A parsed `*.schemas.json`; schema paths are joined onto the config's directory.
#[derive(Debug, Clone)]
pub struct Config {
    pub schemas: Vec<Schema>,
    pub resources: Vec<Resource>,
}

fn staged_path(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel);
    let climbs = rel.split(['/', '\\']).any(|part| part == "..");
    if rel.is_empty() || path.is_absolute() || rel.starts_with('/') || climbs {
        return Err(format!(
            "schema path '{rel}' must be relative to the config"
        ));
    }
    Ok(dir.join(rel))
}

fn check_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(())
    } else {
        Err(format!("url '{url}' must be an http(s) URL"))
    }
}

/// Reads the config. Every schema path is relative to the config's own directory.
pub fn parse_config(path: &Path, bytes: &[u8]) -> Result<Config, String> {
    let shown = path.display();
    let raw: RawConfig = serde_json::from_slice(bytes)
        .map_err(|err| format!("{shown}: not a JSON Schema config: {err}"))?;
    if raw.schema_version != CONFIG_VERSION {
        return Err(format!(
            "{shown}: schema_version {} is not supported (expected {CONFIG_VERSION})",
            raw.schema_version
        ));
    }
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let mut schemas = Vec::new();
    let mut resources: Vec<Resource> = Vec::new();
    let mut add = |resource: Resource| -> Result<(), String> {
        match resources.iter().find(|known| known.path == resource.path) {
            Some(known) if known.url != resource.url => {
                if known.url.is_none() {
                    resources.retain(|r| r.path != resource.path);
                    resources.push(resource);
                    Ok(())
                } else if resource.url.is_none() {
                    Ok(())
                } else {
                    Err(format!(
                        "{shown}: '{}' is declared with two urls",
                        resource.path.display()
                    ))
                }
            }
            Some(_) => Ok(()),
            None => {
                resources.push(resource);
                Ok(())
            }
        }
    };
    for entry in raw.schemas {
        let schema_path =
            staged_path(dir, &entry.schema).map_err(|err| format!("{shown}: {err}"))?;
        if let Some(url) = &entry.url {
            check_url(url).map_err(|err| format!("{shown}: {err}"))?;
        }
        if entry.files.is_empty() && entry.url.is_none() {
            return Err(format!(
                "{shown}: schema '{}' needs files, url, or both",
                entry.schema
            ));
        }
        let mut builder = GlobSetBuilder::new();
        for pattern in &entry.files {
            if pattern.is_empty() || pattern.starts_with('/') {
                return Err(format!(
                    "{shown}: files pattern '{pattern}' must be workspace-relative"
                ));
            }
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|err| format!("{shown}: files pattern '{pattern}': {err}"))?;
            builder.add(glob);
        }
        let matcher = builder
            .build()
            .map_err(|err| format!("{shown}: files patterns: {err}"))?;
        add(Resource {
            path: schema_path.clone(),
            url: entry.url.clone(),
        })?;
        schemas.push(Schema {
            path: schema_path,
            url: entry.url,
            globs: entry.files,
            matcher,
        });
    }
    for entry in raw.resources {
        let resource_path =
            staged_path(dir, &entry.schema).map_err(|err| format!("{shown}: {err}"))?;
        if let Some(url) = &entry.url {
            check_url(url).map_err(|err| format!("{shown}: {err}"))?;
        }
        add(Resource {
            path: resource_path,
            url: entry.url,
        })?;
    }
    Ok(Config { schemas, resources })
}

/// One file the lint run offers: workspace path with `/` separators, filetype and bytes.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub path: &'a str,
    pub filetype: Filetype,
    pub bytes: &'a [u8],
}

/// The files one `check-jsonschema` run validates against one schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub schema: usize,
    pub filetype: Filetype,
    pub files: Vec<String>,
}

fn marker_value<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let rest = line.split_once(marker)?.1;
    rest.split_whitespace().next()
}

fn unquote(text: &str) -> &str {
    text.trim().trim_matches(|c| c == '"' || c == '\'')
}

/// The `$schema` URL a file declares itself: a top-level key or an editor comment.
pub fn declared_schema(bytes: &[u8], filetype: Filetype) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    match filetype {
        Filetype::Json => match serde_json::from_str::<Value>(text).ok()? {
            Value::Object(map) => map.get("$schema")?.as_str().map(str::to_owned),
            _ => None,
        },
        Filetype::Yaml => {
            let mut leading = true;
            for line in text.lines() {
                let trimmed = line.trim();
                if leading && trimmed.starts_with('#') {
                    if let Some(url) = marker_value(trimmed, "yaml-language-server:")
                        .and_then(|_| marker_value(trimmed, "$schema="))
                    {
                        return Some(url.to_owned());
                    }
                    continue;
                }
                if trimmed.is_empty() || trimmed == "---" {
                    continue;
                }
                leading = false;
                for key in ["$schema:", "\"$schema\":", "'$schema':"] {
                    if let Some(value) = line.strip_prefix(key) {
                        let value = value.split(" #").next().unwrap_or(value);
                        return Some(unquote(value).to_owned());
                    }
                }
            }
            None
        }
        Filetype::Toml => {
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("#:schema") {
                    if let Some(url) = rest.split_whitespace().next() {
                        return Some(url.to_owned());
                    }
                }
            }
            let doc = toml_edit::Document::parse(text).ok()?;
            doc.as_item()
                .as_table()?
                .get("$schema")?
                .as_str()
                .map(str::to_owned)
        }
    }
}

/// Groups files by (schema, filetype). A file is picked by a glob, or by declaring a
/// `$schema` URL equal to a declared `url`. Undeclared or relative `$schema` values are
/// ignored. Each (file, schema) pair is validated once.
pub fn select(config: &Config, candidates: &[Candidate<'_>]) -> Vec<Group> {
    let mut groups: BTreeMap<(usize, Filetype), BTreeSet<String>> = BTreeMap::new();
    for candidate in candidates {
        let declared = declared_schema(candidate.bytes, candidate.filetype);
        for (index, schema) in config.schemas.iter().enumerate() {
            let by_glob = schema.matcher.is_match(candidate.path);
            let by_url = match (&declared, &schema.url) {
                (Some(declared), Some(url)) => declared == url,
                _ => false,
            };
            if by_glob || by_url {
                groups
                    .entry((index, candidate.filetype))
                    .or_default()
                    .insert(candidate.path.to_owned());
            }
        }
    }
    groups
        .into_iter()
        .map(|((schema, filetype), files)| Group {
            schema,
            filetype,
            files: files.into_iter().collect(),
        })
        .collect()
}

/// Fails unless the schema's `$schema` is a supported draft. A missing `$schema` is 2020-12.
pub fn draft_gate(path: &str, schema: &Value) -> Result<(), String> {
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    let Some(declared) = object.get("$schema") else {
        return Ok(());
    };
    let supported = || DRAFTS.join(", ");
    let Some(declared) = declared.as_str() else {
        return Err(format!(
            "schema {path}: $schema must be a string; supported drafts: {}",
            supported()
        ));
    };
    let bare = declared.strip_suffix('#').unwrap_or(declared);
    if DRAFTS.contains(&bare) {
        Ok(())
    } else {
        Err(format!(
            "schema {path}: unsupported draft '{declared}'; supported drafts: {}",
            supported()
        ))
    }
}

/// Every http(s) URI a schema closure may reference, mapped to its checked-in copy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefMap {
    pub map: BTreeMap<String, PathBuf>,
}

impl RefMap {
    /// The JSON object the `--dx-ref-map` wrapper reads.
    pub fn to_json(&self) -> String {
        let map: BTreeMap<&str, String> = self
            .map
            .iter()
            .map(|(uri, path)| (uri.as_str(), dx_path::posix(path)))
            .collect();
        serde_json::to_string(&map).unwrap_or_default()
    }
}

fn strip_fragment(uri: &str) -> &str {
    uri.split_once('#').map_or(uri, |(head, _)| head)
}

fn spellings(uri: &str) -> Vec<String> {
    let mut out = vec![strip_fragment(uri).to_owned()];
    if let Ok(parsed) = Url::parse(uri) {
        let mut parsed = parsed;
        parsed.set_fragment(None);
        let normal = parsed.to_string();
        if !out.contains(&normal) {
            out.push(normal);
        }
    }
    out
}

fn join(base: &Option<Url>, reference: &str) -> Option<Url> {
    match base {
        Some(base) => base.join(reference).ok(),
        None => Url::parse(reference).ok(),
    }
}

const DATA_KEYWORDS: [&str; 4] = ["enum", "const", "default", "examples"];
const REF_KEYWORDS: [&str; 3] = ["$ref", "$dynamicRef", "$recursiveRef"];

struct Walk {
    draft4: bool,
    ids: Vec<Url>,
    refs: Vec<Url>,
}

impl Walk {
    fn visit(&mut self, value: &Value, base: &Option<Url>) {
        match value {
            Value::Array(items) => items.iter().for_each(|item| self.visit(item, base)),
            Value::Object(map) => {
                let id_key = if self.draft4 { "id" } else { "$id" };
                let mut here = base.clone();
                if let Some(id) = map.get(id_key).and_then(Value::as_str) {
                    if let Some(next) = join(base, id) {
                        self.ids.push(next.clone());
                        here = Some(next);
                    }
                }
                for (key, member) in map {
                    if REF_KEYWORDS.contains(&key.as_str()) {
                        if let Some(reference) = member.as_str() {
                            if let Some(target) = join(&here, reference) {
                                self.refs.push(target);
                            }
                        }
                        continue;
                    }
                    if DATA_KEYWORDS.contains(&key.as_str()) {
                        continue;
                    }
                    self.visit(member, &here);
                }
            }
            _ => {}
        }
    }
}

/// Gates every resource's draft and maps every pinned URI to its staged copy.
///
/// Any http(s) `$ref`, `$dynamicRef` or `$recursiveRef` target, resolved against the nearest
/// `$id`, that has no checked-in copy fails here, before the tool could download it.
pub fn prepare(
    config: &Config,
    read: &dyn Fn(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<RefMap, String> {
    let mut map: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut known: BTreeSet<String> = BTreeSet::new();
    let mut walks: Vec<(String, Walk)> = Vec::new();
    let mut claim = |uri: &str, path: &Path| -> Result<(), String> {
        for spelled in spellings(uri) {
            match map.get(&spelled) {
                Some(owner) if owner != path => {
                    return Err(format!(
                        "{spelled} is provided by both {} and {}",
                        owner.display(),
                        path.display()
                    ));
                }
                _ => {
                    map.insert(spelled.clone(), path.to_path_buf());
                    known.insert(spelled);
                }
            }
        }
        Ok(())
    };
    let mut embedded: BTreeSet<String> = BTreeSet::new();
    for resource in &config.resources {
        let shown = resource.path.display().to_string();
        let bytes = read(&resource.path).map_err(|err| format!("schema {shown}: {err}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|err| format!("schema {shown}: not valid JSON: {err}"))?;
        draft_gate(&shown, &value)?;
        let draft4 = value
            .get("$schema")
            .and_then(Value::as_str)
            .is_some_and(|s| s.starts_with("http://json-schema.org/draft-04/schema"));
        let mut walk = Walk {
            draft4,
            ids: Vec::new(),
            refs: Vec::new(),
        };
        walk.visit(&value, &None);
        if let Some(url) = &resource.url {
            claim(url, &resource.path)?;
        }
        let root_id = value
            .get(if draft4 { "id" } else { "$id" })
            .and_then(Value::as_str)
            .and_then(|id| Url::parse(id).ok());
        if let Some(id) = &root_id {
            claim(id.as_str(), &resource.path)?;
        }
        for id in &walk.ids {
            if Some(id) != root_id.as_ref() {
                embedded.extend(spellings(id.as_str()));
            }
        }
        walks.push((shown, walk));
    }
    known.extend(embedded);
    for (shown, walk) in &walks {
        for target in &walk.refs {
            if !matches!(target.scheme(), "http" | "https") {
                continue;
            }
            let wanted = spellings(target.as_str());
            if wanted.iter().any(|spelled| known.contains(spelled)) {
                continue;
            }
            let uri = strip_fragment(target.as_str());
            return Err(format!(
                "schema {shown} references {uri}, which has no checked-in copy; \
                 add a json_schema(url = \"{uri}\") dep so it is read offline"
            ));
        }
    }
    Ok(RefMap { map })
}

/// A JSON path in the grammar `jsonschema` prints, as a JSON pointer.
///
/// Accepts `$`, `.name` for `[a-zA-Z][a-zA-Z0-9_]*`, `[N]` and `['esc\'aped']`.
pub fn pointer_of(json_path: &str) -> Option<String> {
    let mut rest = json_path.strip_prefix('$')?;
    let mut pointer = String::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let mut chars = after.char_indices();
            let (_, first) = chars.next()?;
            if !first.is_ascii_alphabetic() {
                return None;
            }
            let end = after
                .char_indices()
                .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
                .map_or(after.len(), |(at, _)| at);
            pointer.push('/');
            pointer.push_str(&escape(&after[..end]));
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix("['") {
            let mut name = String::new();
            let mut chars = after.char_indices();
            let end = loop {
                let (at, c) = chars.next()?;
                match c {
                    '\\' => {
                        let (_, next) = chars.next()?;
                        if next != '\\' && next != '\'' {
                            return None;
                        }
                        name.push(next);
                    }
                    '\'' => break at + 1,
                    other => name.push(other),
                }
            };
            let after = &after[end..];
            rest = after.strip_prefix(']')?;
            pointer.push('/');
            pointer.push_str(&escape(&name));
        } else {
            let after = rest.strip_prefix('[')?;
            let (digits, tail) = after.split_once(']')?;
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            pointer.push('/');
            pointer.push_str(digits);
            rest = tail;
        }
    }
    Some(pointer)
}

fn strip_bom(bytes: &[u8]) -> Result<&str, SyntaxAt> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| SyntaxAt::new(1, 0, "the file is not UTF-8"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

/// The syntax problem a file has as `filetype`, with its position, or `None` when it parses.
///
/// Multi-document YAML is reported as a dx message: `check-jsonschema` reads one document.
pub fn syntax_problem(bytes: &[u8], filetype: Filetype) -> Option<(u64, u64, String)> {
    let text = match strip_bom(bytes) {
        Ok(text) => text,
        Err(err) => return Some((1, 1, err.detail)),
    };
    let at = |line: usize, col: usize, detail: String| (line.max(1) as u64, col as u64 + 1, detail);
    match filetype {
        Filetype::Json => serde_json::from_str::<Value>(text).err().map(|err| {
            at(err.line(), err.column().saturating_sub(1), err.to_string())
        }),
        Filetype::Yaml => match pointer_index::yaml_documents(text) {
            Err(err) => Some(at(err.line, err.col, err.detail)),
            Ok(docs) if docs.len() > 1 => Some((
                docs[1].line,
                docs[1].column,
                "multi-document YAML is not supported by check-jsonschema; keep one document per file"
                    .to_owned(),
            )),
            Ok(_) => None,
        },
        Filetype::Toml => pointer_index::toml_index(text)
            .err()
            .map(|err| at(err.line, err.col, err.detail)),
    }
}

/// The pointer index of a file read as `filetype`.
pub fn index_for(bytes: &[u8], filetype: Filetype) -> Result<Index, SyntaxAt> {
    let text = strip_bom(bytes)?;
    match filetype {
        Filetype::Json => Ok(pointer_index::json_index(text)),
        Filetype::Yaml => pointer_index::yaml_documents(text)?
            .into_iter()
            .next()
            .ok_or_else(|| SyntaxAt::new(1, 0, "no YAML document")),
        Filetype::Toml => pointer_index::toml_index(text),
    }
}

/// The one-based line and char-based column of a `jsonschema` JSON path in a file.
///
/// A path that cannot be read, or a file that does not parse, lands on line 1.
pub fn locate(bytes: &[u8], filetype: Filetype, json_path: &str) -> (u64, u64) {
    let Some(pointer) = pointer_of(json_path) else {
        return (1, 1);
    };
    match index_for(bytes, filetype) {
        Ok(index) => index.locate(&pointer),
        Err(_) => (1, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(json: &str) -> Config {
        parse_config(Path::new("cfg/x.schemas.json"), json.as_bytes()).expect("config")
    }

    fn cand<'a>(path: &'a str, bytes: &'a [u8]) -> Candidate<'a> {
        Candidate {
            path,
            filetype: Filetype::from_path(path).expect("type"),
            bytes,
        }
    }

    #[test]
    fn config_paths_join_the_config_directory() {
        let c = config(
            r#"{"schema_version":1,"schemas":[{"schema":"r/a.json","files":["c/*.json"],"url":"https://e.test/a.json"}],"resources":[{"schema":"r/b.json"}]}"#,
        );
        assert_eq!(c.schemas[0].path, Path::new("cfg/r/a.json"));
        assert_eq!(c.resources.len(), 2);
        assert_eq!(c.resources[0].url.as_deref(), Some("https://e.test/a.json"));
    }

    #[test]
    fn config_rejects_bad_input() {
        let bad = |json: &str| parse_config(Path::new("c.json"), json.as_bytes()).unwrap_err();
        assert!(bad(r#"{"schema_version":2,"schemas":[]}"#).contains("schema_version"));
        assert!(
            bad(r#"{"schema_version":1,"schemas":[{"schema":"../a.json","files":["x"]}]}"#)
                .contains("relative")
        );
        assert!(
            bad(r#"{"schema_version":1,"schemas":[{"schema":"a.json"}]}"#).contains("needs files")
        );
        assert!(
            bad(r#"{"schema_version":1,"schemas":[{"schema":"a.json","url":"ftp://x"}]}"#)
                .contains("http(s)")
        );
        assert!(bad("nope").contains("not a JSON Schema config"));
    }

    #[test]
    fn globs_respect_separators() {
        let c = config(
            r#"{"schema_version":1,"schemas":[{"schema":"a.json","files":["conf/*.json","deep/**/*.yaml"]}]}"#,
        );
        let files = [
            cand("conf/a.json", b"{}"),
            cand("conf/sub/b.json", b"{}"),
            cand("deep/x/y/z.yaml", b"a: 1"),
            cand("deep/z.yaml", b"a: 1"),
            cand("other/z.yaml", b"a: 1"),
        ];
        let groups = select(&c, &files);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].files, ["conf/a.json"]);
        assert_eq!(groups[1].filetype, Filetype::Yaml);
        assert_eq!(groups[1].files, ["deep/x/y/z.yaml", "deep/z.yaml"]);
    }

    #[test]
    fn schema_pickup_needs_a_declared_url_and_counts_once() {
        let c = config(
            r#"{"schema_version":1,"schemas":[{"schema":"a.json","files":["a/*.json"],"url":"https://e.test/a.json"}]}"#,
        );
        let json = br#"{"$schema":"https://e.test/a.json"}"#;
        let other = br#"{"$schema":"https://e.test/other.json"}"#;
        let relative = br#"{"$schema":"./a.json"}"#;
        let files = [
            cand("a/both.json", json),
            cand("z/by-url.json", json),
            cand("z/other.json", other),
            cand("z/relative.json", relative),
        ];
        let groups = select(&c, &files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files, ["a/both.json", "z/by-url.json"]);
    }

    #[test]
    fn declared_schema_in_each_format() {
        assert_eq!(
            declared_schema(
                b"# yaml-language-server: $schema=https://e.test/y.json\na: 1\n",
                Filetype::Yaml
            )
            .as_deref(),
            Some("https://e.test/y.json")
        );
        assert_eq!(
            declared_schema(
                b"---\n\"$schema\": \"https://e.test/k.json\"\n",
                Filetype::Yaml
            )
            .as_deref(),
            Some("https://e.test/k.json")
        );
        assert_eq!(
            declared_schema(b"#:schema https://e.test/t.json\na = 1\n", Filetype::Toml).as_deref(),
            Some("https://e.test/t.json")
        );
        assert_eq!(
            declared_schema(b"\"$schema\" = \"https://e.test/u.json\"\n", Filetype::Toml)
                .as_deref(),
            Some("https://e.test/u.json")
        );
        assert_eq!(declared_schema(b"[1]", Filetype::Json), None);
        assert_eq!(declared_schema(b"a: 1\n", Filetype::Yaml), None);
    }

    #[test]
    fn every_draft_spelling_passes_and_others_fail() {
        for draft in DRAFTS {
            for suffix in ["", "#"] {
                let schema = serde_json::json!({"$schema": format!("{draft}{suffix}")});
                assert!(draft_gate("s.json", &schema).is_ok(), "{draft}{suffix}");
            }
        }
        assert!(draft_gate("s.json", &serde_json::json!({})).is_ok());
        assert!(draft_gate("s.json", &serde_json::json!(true)).is_ok());
        for bad in [
            "http://json-schema.org/draft-03/schema#",
            "https://json-schema.org/draft-07/schema",
            "https://example.com/meta",
        ] {
            let err = draft_gate("s.json", &serde_json::json!({"$schema": bad})).unwrap_err();
            assert!(
                err.contains("s.json") && err.contains("supported drafts"),
                "{err}"
            );
        }
        assert!(draft_gate("s.json", &serde_json::json!({"$schema": 4})).is_err());
    }

    fn prep(config_json: &str, files: &[(&str, &str)]) -> Result<RefMap, String> {
        let c = config(config_json);
        let store: BTreeMap<PathBuf, String> = files
            .iter()
            .map(|(p, b)| (PathBuf::from(p), (*b).to_owned()))
            .collect();
        prepare(&c, &|path| {
            store
                .get(path)
                .map(|b| b.clone().into_bytes())
                .ok_or_else(|| std::io::Error::other("missing"))
        })
    }

    const ID_CONFIG: &str = r#"{"schema_version":1,"schemas":[{"schema":"a.json","files":["x"]}],"resources":[{"schema":"dep.json","url":"https://e.test/s/dep.json"}]}"#;

    #[test]
    fn id_relative_ref_to_a_pinned_dep_is_allowed() {
        let a =
            r##"{"$id":"https://e.test/s/a.json","properties":{"n":{"$ref":"dep.json#/defs/x"}}}"##;
        let map = prep(ID_CONFIG, &[("cfg/a.json", a), ("cfg/dep.json", "{}")]).expect("ok");
        assert_eq!(
            map.map["https://e.test/s/dep.json"],
            Path::new("cfg/dep.json")
        );
        assert_eq!(map.map["https://e.test/s/a.json"], Path::new("cfg/a.json"));
        assert!(map.to_json().contains("https://e.test/s/dep.json"));
    }

    #[test]
    fn id_relative_ref_to_an_unpinned_url_is_rejected() {
        let a = r#"{"$id":"https://e.test/s/a.json","properties":{"n":{"$ref":"other.json"}}}"#;
        let err = prep(ID_CONFIG, &[("cfg/a.json", a), ("cfg/dep.json", "{}")]).unwrap_err();
        assert!(err.contains("https://e.test/s/other.json"), "{err}");
        assert!(err.contains("json_schema(url ="), "{err}");
    }

    #[test]
    fn nested_ids_and_data_keywords_are_understood() {
        let a = r##"{"$id":"https://e.test/s/a.json","$defs":{"inner":{"$id":"https://e.test/s/inner.json","type":"string"}},
          "properties":{"n":{"$ref":"https://e.test/s/inner.json"},"m":{"$ref":"#/$defs/inner"}},
          "default":{"$ref":"https://nowhere.test/x"},"enum":[{"$ref":"https://nowhere.test/y"}]}"##;
        assert!(prep(ID_CONFIG, &[("cfg/a.json", a), ("cfg/dep.json", "{}")]).is_ok());
    }

    #[test]
    fn dynamic_and_local_refs() {
        let a = r##"{"properties":{"n":{"$ref":"dep.json"},"m":{"$dynamicRef":"https://nowhere.test/z"}}}"##;
        let err = prep(ID_CONFIG, &[("cfg/a.json", a), ("cfg/dep.json", "{}")]).unwrap_err();
        assert!(err.contains("https://nowhere.test/z"), "{err}");
        let ok = r##"{"properties":{"n":{"$ref":"dep.json"},"m":{"$ref":"#/properties/n"}}}"##;
        assert!(prep(ID_CONFIG, &[("cfg/a.json", ok), ("cfg/dep.json", "{}")]).is_ok());
    }

    #[test]
    fn the_gate_covers_every_resource() {
        let dep = r#"{"$schema":"http://json-schema.org/draft-03/schema#"}"#;
        let err = prep(ID_CONFIG, &[("cfg/a.json", "{}"), ("cfg/dep.json", dep)]).unwrap_err();
        assert!(
            err.contains("cfg/dep.json") && err.contains("draft-03"),
            "{err}"
        );
    }

    #[test]
    fn json_path_grammar() {
        assert_eq!(pointer_of("$").as_deref(), Some(""));
        assert_eq!(pointer_of("$.a.b_1[0].c").as_deref(), Some("/a/b_1/0/c"));
        assert_eq!(pointer_of("$['a.b']['x[0]']").as_deref(), Some("/a.b/x[0]"));
        assert_eq!(
            pointer_of(r"$['it\'s']['a\\b']").as_deref(),
            Some(r"/it's/a\b")
        );
        assert_eq!(pointer_of("$['a/b~']").as_deref(), Some("/a~1b~0"));
        assert_eq!(pointer_of("$[0]"), pointer_of("$['0']"));
        assert_eq!(pointer_of("a"), None);
        assert_eq!(pointer_of("$.1a"), None);
        assert_eq!(pointer_of("$[x]"), None);
        assert_eq!(pointer_of("$['open"), None);
    }

    #[test]
    fn locate_in_json_yaml_and_toml() {
        let json = b"{\n  \"a\": {\n    \"b\": [1,\n      \"x\"]\n  }\n}\n";
        assert_eq!(locate(json, Filetype::Json, "$.a.b[1]"), (4, 7));
        assert_eq!(locate(json, Filetype::Json, "$.a.zz"), (2, 3));
        let yaml = b"a:\n  b:\n    - 1\n    - x: &k 2\n      y: *k\n  \"d.e\": 3\n";
        assert_eq!(locate(yaml, Filetype::Yaml, "$.a.b[1].x"), (4, 7));
        assert_eq!(locate(yaml, Filetype::Yaml, "$.a.b[1].y"), (5, 7));
        assert_eq!(locate(yaml, Filetype::Yaml, "$.a['d.e']"), (6, 3));
        let toml = "title = 1\n[srv]\nport = 1\n[[hosts]]\nname = \"a\"\n[[hosts]]\nname = \"b\"\n[t]\nx.y = 2\ninl = { k = 1, j = [1, 2] }\nm = \"\"\"\nl1\nl2\"\"\"\nn = 3\n";
        let at = |p: &str| locate(toml.as_bytes(), Filetype::Toml, p);
        assert_eq!(at("$.srv.port"), (3, 1));
        assert_eq!(at("$.hosts[1].name"), (7, 1));
        assert_eq!(at("$.hosts[0]").0, 4);
        assert_eq!(at("$.t.x.y"), (9, 3));
        assert_eq!(at("$.t.inl.j[1]"), (10, 24));
        assert_eq!(at("$.t.n"), (14, 1));
    }

    #[test]
    fn locate_counts_chars_not_bytes() {
        let yaml = "bl\u{e5}: 1\nkey: { \u{f8}: 2, b: 3 }\n";
        assert_eq!(locate(yaml.as_bytes(), Filetype::Yaml, "$.key.b"), (2, 14));
        assert_eq!(
            locate(yaml.as_bytes(), Filetype::Yaml, "$['bl\u{e5}']"),
            (1, 1)
        );
        let toml = "\"\u{e5}\u{e5}\" = 1\nz = 2\nw = [\"\u{e5}\", 1]\n";
        assert_eq!(locate(toml.as_bytes(), Filetype::Toml, "$.w[1]"), (3, 11));
    }

    #[test]
    fn syntax_problems_have_positions() {
        let (line, _, _) = syntax_problem(b"{\n  \"a\": ,\n}", Filetype::Json).expect("json");
        assert_eq!(line, 2);
        let (line, _, _) = syntax_problem(b"a: 1\nb: [\n", Filetype::Yaml).expect("yaml");
        assert!(line >= 2);
        let (line, _, _) = syntax_problem(b"a = 1\nb = \n", Filetype::Toml).expect("toml");
        assert!(line >= 2);
        let (line, _, message) =
            syntax_problem(b"a: 1\n---\na: 2\n", Filetype::Yaml).expect("multi");
        assert_eq!(line, 3);
        assert!(message.contains("multi-document"));
        assert!(syntax_problem(b"a: 1\n", Filetype::Yaml).is_none());
        assert!(syntax_problem(&[0xff], Filetype::Json).is_some());
    }

    #[test]
    fn locate_falls_back_when_the_file_does_not_parse() {
        assert_eq!(locate(b"{", Filetype::Json, "$.a"), (1, 1));
        assert_eq!(locate(b"a: 1", Filetype::Yaml, "bogus"), (1, 1));
    }
}
