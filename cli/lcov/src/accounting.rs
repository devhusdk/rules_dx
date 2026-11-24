use std::collections::{BTreeMap, BTreeSet};

use crate::lang::{LineComment, has_suffix, line_comment_syntax};
use crate::{
    FileHits, Ignores, LcovError, find_ignores, is_covered_language, is_ignored, merge_lcov_reports,
};

pub const SOURCES_ASPECT: &str = "core/quality:coverage_sources.bzl%dx_coverage_sources_aspect";
pub const SOURCES_OUTPUT_GROUP: &str = "dx_coverage_sources";

const MANIFEST_VERSION: &str = "v2";
const UNKNOWN_CELL: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub path: String,
    pub test: bool,
    pub generated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceManifest {
    pub cell: String,
    pub rows: Vec<SourceRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Eligible,
    Support,
    Excluded,
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellAccount {
    pub cell: String,
    pub raw_lines: u64,
    pub excluded_lines: u64,
    pub eligible_lines: u64,
    pub covered_lines: u64,
    pub eligible_files: u64,
    pub support_files: u64,
    pub excluded_files: u64,
    pub unsupported_files: u64,
    pub files: BTreeMap<String, Disposition>,
    pub file_lines: BTreeMap<String, (u64, u64)>,
    pub absent: Vec<String>,
    pub orphans: Vec<String>,
    pub errors: Vec<String>,
}

impl CellAccount {
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "coverage sources {}: {} raw, {} excluded, {} eligible, {} covered lines; {} eligible, {} support, {} excluded, {} unsupported files",
            self.cell,
            self.raw_lines,
            self.excluded_lines,
            self.eligible_lines,
            self.covered_lines,
            self.eligible_files,
            self.support_files,
            self.excluded_files,
            self.unsupported_files,
        )
    }

    pub fn record_orphans(&mut self, paths: impl IntoIterator<Item = String>) {
        let found: BTreeSet<String> = paths.into_iter().collect();
        for path in found {
            self.errors.push(format!(
                "tracked source not owned by any Bazel target: {path}"
            ));
            self.orphans.push(path);
        }
    }

    #[must_use]
    pub fn meets(&self, minimum: u32) -> bool {
        self.errors.is_empty()
            && self.eligible_lines > 0
            && self.covered_lines * 100 >= u64::from(minimum) * self.eligible_lines
    }
}

#[derive(Debug, Default)]
pub(crate) struct LineCount {
    pub(crate) covered: u64,
    pub(crate) eligible: u64,
    pub(crate) ignored: u64,
    pub(crate) uncovered: Vec<u32>,
}

pub(crate) fn count_lines(hits: &FileHits, ignores: &Ignores) -> LineCount {
    let mut count = LineCount::default();
    let mut numbered: Vec<u32> = hits.lines.keys().copied().collect();
    numbered.sort_unstable();
    for line in numbered {
        if is_ignored(ignores, line) {
            count.ignored += 1;
            continue;
        }
        count.eligible += 1;
        if hits.lines.get(&line).is_some_and(|hit| *hit > 0) {
            count.covered += 1;
        } else {
            count.uncovered.push(line);
        }
    }
    count
}

#[must_use]
pub fn orphan_candidates(tracked: &[String], ignored_prefixes: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tracked
        .iter()
        .filter(|path| is_covered_language(path))
        .filter(|path| *path != dx_path::state::DX_DIR && !path.starts_with(".dx/"))
        .filter(|path| {
            !ignored_prefixes.iter().any(|prefix| {
                let prefix = prefix.trim_start_matches("./").trim_end_matches('/');
                !prefix.is_empty()
                    && (path.as_str() == prefix
                        || path
                            .strip_prefix(prefix)
                            .is_some_and(|rest| rest.starts_with('/')))
            })
        })
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn source_key(path: &str) -> String {
    let normalized = normalize_path(path);
    if let Some(rest) = normalized.strip_prefix("bazel-out/")
        && let Some((_, tail)) = rest.split_once("/bin/")
    {
        return tail.to_string();
    }
    normalized
}

const DART_ROOT_URI: &str = "org-dartlang-root:///";

pub(crate) fn resolve_dart_uris(document: &str, owned: &BTreeSet<String>) -> String {
    let mut out = String::with_capacity(document.len());
    let mut dropping = false;
    for line in document.split_inclusive('\n') {
        if let Some(uri) = line.strip_prefix("SF:") {
            match resolve_dart_uri(uri.trim_end(), owned) {
                Resolved::Path(path) => {
                    out.push_str(&format!("SF:{path}\n"));
                    continue;
                }
                Resolved::Dependency => dropping = true,
                Resolved::Other => {}
            }
        }
        if !dropping {
            out.push_str(line);
        } else if line.trim() == "end_of_record" {
            dropping = false;
        }
    }
    out
}

enum Resolved {
    Path(String),
    Dependency,
    Other,
}

fn resolve_dart_uri(uri: &str, owned: &BTreeSet<String>) -> Resolved {
    if let Some(path) = uri.strip_prefix(DART_ROOT_URI) {
        return Resolved::Path(path.to_string());
    }
    let Some((_, rest)) = uri
        .strip_prefix("package:")
        .and_then(|rest| rest.split_once('/'))
    else {
        return Resolved::Other;
    };
    let lib = format!("lib/{rest}");
    let suffix = format!("/{lib}");
    let mut found = owned
        .iter()
        .filter(|key| **key == lib || key.ends_with(&suffix));
    match (found.next(), found.next()) {
        (Some(key), None) => Resolved::Path(key.clone()),
        _ => Resolved::Dependency,
    }
}

pub fn parse_source_manifest(text: &str) -> Result<SourceManifest, dx_error::DxError> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(' ').collect();
    let cell = match header.as_slice() {
        [MANIFEST_VERSION, "cell", cell] if !cell.is_empty() => (*cell).to_string(),
        _ => {
            return Err(dx_error::DxError::failure(format!(
                "unsupported source manifest header: {:?}",
                header.join(" ")
            )));
        }
    };
    if cell == UNKNOWN_CELL {
        return Err(dx_error::DxError::failure(
            "source manifest names an unknown cell",
        ));
    }
    let mut manifest = SourceManifest {
        cell,
        rows: Vec::new(),
    };
    let mut unpaired: BTreeMap<String, usize> = BTreeMap::new();
    for (index, line) in lines.enumerate() {
        let lineno = index + 2;
        let fields: Vec<&str> = line.splitn(3, ' ').collect();
        let [kind, origin, path] = fields.as_slice() else {
            return Err(dx_error::DxError::failure(format!(
                "malformed source manifest row {lineno}: {line:?}"
            )));
        };
        if *kind == "cell" {
            return Err(dx_error::DxError::failure(format!(
                "source manifest names more than one cell at row {lineno}"
            )));
        }
        let test = match *kind {
            "prod" => false,
            "test" => true,
            _ => {
                return Err(dx_error::DxError::failure(format!(
                    "unknown source kind {kind:?} at row {lineno}"
                )));
            }
        };
        let generated = match *origin {
            "source" => false,
            "generated" => true,
            _ => {
                return Err(dx_error::DxError::failure(format!(
                    "unknown source origin {origin:?} at row {lineno}"
                )));
            }
        };
        if path.is_empty() {
            return Err(dx_error::DxError::failure(format!(
                "malformed source manifest row {lineno}: {line:?}"
            )));
        }
        let row = SourceRow {
            path: (*path).to_string(),
            test,
            generated,
        };
        let pair = unpaired
            .get(*path)
            .and_then(|&at| manifest.rows.get_mut(at))
            .filter(|held| held.test != test && held.generated == generated);
        if let Some(held) = pair {
            held.test = false;
            unpaired.remove(*path);
        } else {
            unpaired.insert((*path).to_string(), manifest.rows.len());
            manifest.rows.push(row);
        }
    }
    let paths: Vec<&str> = manifest.rows.iter().map(|row| row.path.as_str()).collect();
    let mut errors = Vec::new();
    alias_errors("source manifest", &paths, &mut errors);
    match errors.into_iter().next() {
        Some(error) => Err(dx_error::DxError::failure(error)),
        None => Ok(manifest),
    }
}

const DECLARATION_SUFFIXES: &[&str] = &[".d.ts", ".d.mts", ".d.cts"];
const HEADER_SUFFIXES: &[&str] = &[".h", ".hh", ".hpp", ".hxx"];
const UNCOLLECTED: &[(&str, &[&str])] = &[(
    "windows_",
    &[".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx"],
)];

fn collected_in(cell: &str, path: &str) -> bool {
    !UNCOLLECTED.iter().any(|(prefix, suffixes)| {
        cell.starts_with(prefix) && suffixes.iter().any(|suffix| has_suffix(path, suffix))
    })
}

fn is_declaration_stub(path: &str) -> bool {
    DECLARATION_SUFFIXES
        .iter()
        .any(|suffix| has_suffix(path, suffix))
}

fn is_header(path: &str) -> bool {
    HEADER_SUFFIXES
        .iter()
        .any(|suffix| has_suffix(path, suffix))
}

fn read_error(path: &str, error: &LcovError) -> String {
    format!("cannot read {path} for LCOV_EXCL markers: {error}")
}

fn whole_file_excluded(path: &str, source: &str) -> bool {
    let Ok(ignores) = find_ignores(path, source) else {
        return false;
    };
    if ignores.ranges.is_empty() {
        return false;
    }
    let marker = match line_comment_syntax(path) {
        Some(LineComment::Hash) => "#",
        Some(LineComment::Percent) => "%",
        Some(LineComment::DashDash) => "--",
        Some(LineComment::ParenStar) => "(*",
        _ => "//",
    };
    source.lines().zip(1_u32..).all(|(line, number)| {
        let text = line.trim();
        text.is_empty() || text.starts_with(marker) || is_ignored(&ignores, number)
    })
}

fn ignore_hygiene_errors(path: &str, hits: &FileHits, ignores: &Ignores) -> Vec<String> {
    let mut errors = Vec::new();
    for line in ignores.singles.keys() {
        if !hits.lines.contains_key(line) {
            errors.push(format!(
                "stale coverage ignore at {path}:{line}: no instrumented line; remove it"
            ));
        }
    }
    for (start, stop, _) in &ignores.ranges {
        if !hits.lines.keys().any(|line| start <= line && line <= stop) {
            errors.push(format!(
                "stale coverage range at {path}:{start}-{stop}: covers no instrumented lines; remove it"
            ));
        }
    }
    if hits.lines.keys().all(|line| is_ignored(ignores, *line)) {
        errors.push(format!(
            "broad coverage ignore at {path}: all {} instrumented lines ignored; narrow the range or reclassify to support",
            hits.lines.len()
        ));
    }
    errors
}

pub fn account(
    manifests: &[SourceManifest],
    lcov_documents: &[String],
    load_source: &dyn Fn(&str) -> Result<String, LcovError>,
) -> CellAccount {
    let mut result = CellAccount::default();
    let cells: BTreeSet<&str> = manifests.iter().map(|m| m.cell.as_str()).collect();
    result.cell = cells
        .iter()
        .next()
        .copied()
        .unwrap_or(UNKNOWN_CELL)
        .to_string();
    if manifests.is_empty() {
        result
            .errors
            .push("no coverage source manifests were reported".to_string());
    }
    if cells.len() > 1 {
        let names: Vec<&str> = cells.into_iter().collect();
        result.errors.push(format!(
            "coverage source manifests name more than one cell: {}",
            names.join(", ")
        ));
    }
    let mut owned: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for row in manifests.iter().flat_map(|m| &m.rows) {
        let slot = owned
            .entry(source_key(&row.path))
            .or_insert_with(|| (row.path.clone(), row.test));
        slot.1 = slot.1 && row.test;
    }
    let owned_keys: BTreeSet<String> = owned.keys().cloned().collect();
    let lcov_documents: Vec<String> = lcov_documents
        .iter()
        .map(|document| resolve_dart_uris(document, &owned_keys))
        .collect();
    let report = match merge_lcov_reports(&lcov_documents) {
        Ok(report) => report,
        Err(error) => {
            result
                .errors
                .push(format!("invalid coverage report: {error}"));
            BTreeMap::new()
        }
    };
    let mut reported: BTreeMap<String, FileHits> = BTreeMap::new();
    for (path, hits) in report {
        let key = source_key(&path);
        if !owned.contains_key(&key) {
            result.errors.push(format!(
                "coverage report path not owned by any target: {path}"
            ));
        }
        reported.entry(key).or_default().merge(hits);
    }
    for (key, (path, test)) in &owned {
        let disposition = if *test || is_declaration_stub(path) {
            Disposition::Support
        } else if !is_covered_language(path) || !collected_in(&result.cell, path) {
            Disposition::Unsupported
        } else if let Some(hits) = reported.get(key) {
            if hits.lines.is_empty() {
                Disposition::Support
            } else {
                let ignores = match load_source(path) {
                    Ok(text) => match find_ignores(path, &text) {
                        Ok(valid) => valid,
                        Err(error) => {
                            result.errors.push(error.to_string());
                            Ignores::default()
                        }
                    },
                    Err(error) => {
                        result.errors.push(read_error(path, &error));
                        Ignores::default()
                    }
                };
                result
                    .errors
                    .extend(ignore_hygiene_errors(path, hits, &ignores));
                let count = count_lines(hits, &ignores);
                result.raw_lines += count.eligible + count.ignored;
                result.excluded_lines += count.ignored;
                result.eligible_lines += count.eligible;
                result.covered_lines += count.covered;
                result
                    .file_lines
                    .insert(path.clone(), (count.covered, count.eligible));
                Disposition::Eligible
            }
        } else if whole_file_excluded(
            path,
            &load_source(path).unwrap_or_else(|error| {
                result.errors.push(read_error(path, &error));
                String::new()
            }),
        ) {
            Disposition::Excluded
        } else if is_header(path) {
            Disposition::Support
        } else {
            result.errors.push(format!(
                "production source absent from coverage report: {path}"
            ));
            result.absent.push(path.clone());
            continue;
        };
        match disposition {
            Disposition::Eligible => result.eligible_files += 1,
            Disposition::Support => result.support_files += 1,
            Disposition::Excluded => result.excluded_files += 1,
            Disposition::Unsupported => result.unsupported_files += 1,
        }
        result.files.insert(path.clone(), disposition);
    }
    result
}

#[cfg(test)]
#[path = "accounting_tests.rs"]
mod accounting_tests;

fn normalize_path(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    let mut rest = slashed.as_str();
    while let Some(stripped) = rest.strip_prefix("./") {
        rest = stripped;
    }
    rest.to_string()
}

fn alias_errors(label: &str, paths: &[&str], errors: &mut Vec<String>) {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for path in paths {
        let key = normalize_path(path);
        match seen.get(&key) {
            Some(first) if first != path => errors.push(format!(
                "path alias in {label}: {first} and {path} name the same file"
            )),
            Some(_) => errors.push(format!("duplicate path in {label}: {path}")),
            None => {
                seen.insert(key, path);
            }
        }
    }
}
