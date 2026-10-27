use std::collections::{BTreeMap, BTreeSet};

use crate::verdict::{alias_errors, line_comment_syntax, normalize_path, LineComment};
use crate::{
    find_ignores, is_covered_language, is_ignored, merge_lcov_reports, FileHits, Ignores, LcovError,
};

/// The rules_dx-relative aspect that writes one source manifest per top-level target.
pub const SOURCES_ASPECT: &str = "quality:coverage_sources.bzl%dx_coverage_sources_aspect";
/// The output group that carries the source manifests.
pub const SOURCES_OUTPUT_GROUP: &str = "dx_coverage_sources";

const MANIFEST_VERSION: &str = "v1";
const UNKNOWN_CELL: &str = "unknown";

/// One instrumented source a target owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub path: String,
    pub test: bool,
    pub generated: bool,
}

/// The instrumented sources of one top-level target in one cell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceManifest {
    pub cell: String,
    pub rows: Vec<SourceRow>,
}

/// How the accounting treats one owned source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Eligible,
    Support,
    Excluded,
    Unsupported,
}

/// Every owned source of one cell with its disposition, line counts and accounting errors.
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
    /// Production sources owned by an instrumented target that the report never saw.
    pub absent: Vec<String>,
    pub errors: Vec<String>,
}

impl CellAccount {
    /// The one-line counts summary for this cell.
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

    /// True when the cell has no accounting errors, at least one eligible line, and meets `minimum` percent.
    pub fn meets(&self, minimum: u32) -> bool {
        self.errors.is_empty()
            && self.eligible_lines > 0
            && self.covered_lines * 100 >= u64::from(minimum) * self.eligible_lines
    }
}

/// The line counts of one reported file under its validated exclusions.
#[derive(Debug, Default)]
pub(crate) struct LineCount {
    pub(crate) covered: u64,
    pub(crate) eligible: u64,
    pub(crate) ignored: u64,
    pub(crate) uncovered: Vec<u32>,
}

/// Counts one file's reported lines: ignored, eligible, and covered (eligible with hits).
pub(crate) fn count_lines(hits: &FileHits, ignores: &Ignores) -> LineCount {
    let mut count = LineCount::default();
    let mut numbered: Vec<u32> = hits.lines.keys().copied().collect();
    numbered.sort();
    for line in numbered {
        if is_ignored(ignores, line) {
            count.ignored += 1;
            continue;
        }
        count.eligible += 1;
        if hits.lines[&line] > 0 {
            count.covered += 1;
        } else {
            count.uncovered.push(line);
        }
    }
    count
}

/// The path key a manifest row and an LCOV SF record share.
pub(crate) fn source_key(path: &str) -> String {
    let normalized = normalize_path(path);
    if let Some(rest) = normalized.strip_prefix("bazel-out/") {
        if let Some((_, tail)) = rest.split_once("/bin/") {
            return tail.to_string();
        }
    }
    normalized
}

/// Parses one source manifest: a `v1 cell <cell>` header, then `<prod|test> <source|generated> <path>` rows.
pub fn parse_source_manifest(text: &str) -> Result<SourceManifest, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(' ').collect();
    let cell = match header.as_slice() {
        [MANIFEST_VERSION, "cell", cell] if !cell.is_empty() => (*cell).to_string(),
        _ => {
            return Err(format!(
                "unsupported source manifest header: {:?}",
                header.join(" ")
            ))
        }
    };
    if cell == UNKNOWN_CELL {
        return Err("source manifest names an unknown cell".to_string());
    }
    let mut manifest = SourceManifest {
        cell,
        rows: Vec::new(),
    };
    for (index, line) in lines.enumerate() {
        let lineno = index + 2;
        let fields: Vec<&str> = line.splitn(3, ' ').collect();
        let [kind, origin, path] = fields.as_slice() else {
            return Err(format!("malformed source manifest row {lineno}: {line:?}"));
        };
        if *kind == "cell" {
            return Err(format!(
                "source manifest names more than one cell at row {lineno}"
            ));
        }
        let test = match *kind {
            "prod" => false,
            "test" => true,
            _ => return Err(format!("unknown source kind {kind:?} at row {lineno}")),
        };
        let generated = match *origin {
            "source" => false,
            "generated" => true,
            _ => return Err(format!("unknown source origin {origin:?} at row {lineno}")),
        };
        if path.is_empty() {
            return Err(format!("malformed source manifest row {lineno}: {line:?}"));
        }
        manifest.rows.push(SourceRow {
            path: (*path).to_string(),
            test,
            generated,
        });
    }
    let paths: Vec<&str> = manifest.rows.iter().map(|row| row.path.as_str()).collect();
    let mut errors = Vec::new();
    alias_errors("source manifest", &paths, &mut errors);
    match errors.into_iter().next() {
        Some(error) => Err(error),
        None => Ok(manifest),
    }
}

/// Declaration stubs that carry no executable lines.
const DECLARATION_SUFFIXES: &[&str] = &[".d.ts", ".d.mts", ".d.cts"];
/// C and C++ headers, which collectors omit from LCOV when they hold only declarations.
const HEADER_SUFFIXES: &[&str] = &[".h", ".hh", ".hpp", ".hxx"];

/// True for files that hold declarations only and so have nothing to execute.
fn is_declaration_stub(path: &str) -> bool {
    DECLARATION_SUFFIXES
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

/// True for C and C++ headers.
fn is_header(path: &str) -> bool {
    HEADER_SUFFIXES.iter().any(|suffix| path.ends_with(suffix))
}

/// True when a valid exclusion range covers every line that is not blank or a comment.
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
        _ => "//",
    };
    source.lines().enumerate().all(|(index, line)| {
        let text = line.trim();
        text.is_empty() || text.starts_with(marker) || is_ignored(&ignores, index as u32 + 1)
    })
}

/// Accounts every owned source of one cell against the merged LCOV report.
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
    let report = match merge_lcov_reports(lcov_documents) {
        Ok(report) => report,
        Err(error) => {
            result
                .errors
                .push(format!("invalid coverage report: {error}"));
            BTreeMap::new()
        }
    };
    let mut reported = BTreeMap::new();
    for (path, hits) in &report {
        let key = source_key(path);
        if !owned.contains_key(&key) {
            result.errors.push(format!(
                "coverage report path not owned by any target: {path}"
            ));
        }
        reported.insert(key, hits);
    }
    for (key, (path, test)) in &owned {
        let source = || load_source(path).unwrap_or_default();
        let disposition = if *test || is_declaration_stub(path) {
            Disposition::Support
        } else if !is_covered_language(path) {
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
                    Err(_) => Ignores::default(),
                };
                let count = count_lines(hits, &ignores);
                result.raw_lines += count.eligible + count.ignored;
                result.excluded_lines += count.ignored;
                result.eligible_lines += count.eligible;
                result.covered_lines += count.covered;
                Disposition::Eligible
            }
        } else if whole_file_excluded(path, &source()) {
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
