use std::collections::BTreeMap;

pub const SEED_CELL: &str = "seed-linux_x86_64";

pub const CELLS: &[(&str, &str)] = &[
    ("seed-linux_x86_64", "tools/coverage/seed-inventory.txt"),
    ("linux_arm64", "tools/coverage/arm64-inventory.txt"),
    ("macos_arm64", "tools/coverage/macos-arm64-inventory.txt"),
    ("windows_x86_64", "tools/coverage/windows-x86_64-inventory.txt"),
    ("windows_arm64", "tools/coverage/windows-arm64-inventory.txt"),
];

pub const CLASS_PROD: &str = "prod";
pub const CLASS_TEST: &str = "test";
pub const CLASS_EXAMPLE: &str = "example";
pub const CLASS_GENERATED: &str = "generated";

pub const ELIGIBLE: &str = "eligible";
pub const SUPPORT: &str = "support";

pub const REASON_FULLY_COVERED: &str = "fully-covered";
pub const REASON_TEST: &str = "test-source";
pub const REASON_EXAMPLE: &str = "example-source";
pub const REASON_GENERATED: &str = "generated-source";
pub const REASON_DECLARATION: &str = "declaration-only";
pub const REASON_NOT_APPLICABLE: &str = "not-applicable-to-cell";
pub const REASON_UNMEASURED_LANGUAGE: &str = "unmeasured-language";
pub const REASON_UNCOVERED: &str = "uncovered-lines";
pub const REASON_NO_LINES: &str = "no-executable-lines";
pub const REASON_ABSENT: &str = "absent-from-report";

pub const REASONS: &[&str] = &[
    REASON_FULLY_COVERED,
    REASON_TEST,
    REASON_EXAMPLE,
    REASON_GENERATED,
    REASON_DECLARATION,
    REASON_NOT_APPLICABLE,
    REASON_UNMEASURED_LANGUAGE,
    REASON_UNCOVERED,
    REASON_NO_LINES,
    REASON_ABSENT,
];

pub const DECLARATION_SUFFIXES: &[&str] = &[".d.ts", ".d.mts", ".d.cts"];

pub const SEED_MEASURED_EXTENSIONS: &[&str] = &[".rs", ".go"];

pub fn extract_covered(text: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut covered = Vec::new();
    let mut declarations = Vec::new();
    let mut block = "";
    for raw in text.lines() {
        let line = raw.trim();
        if line.contains("COVERED_LANGUAGES:") {
            block = "covered";
            continue;
        }
        if line.contains("DECLARATION_SUFFIXES:") {
            block = "declarations";
            continue;
        }
        if line == "];" {
            block = "";
            continue;
        }
        if block.is_empty() {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            let end = after.find('"').unwrap_or(after.len());
            let literal = &after[..end];
            if literal.starts_with('.') {
                if block == "covered" {
                    covered.push(literal.to_string());
                } else {
                    declarations.push(literal.to_string());
                }
            }
            rest = &after[end.min(after.len())..];
            if end >= after.len() {
                break;
            }
            rest = &rest[1.min(rest.len())..];
        }
    }
    if covered.is_empty() {
        return Err("no covered extensions parsed from the language table".to_string());
    }
    Ok((covered, declarations))
}

pub fn is_declaration(path: &str) -> bool {
    DECLARATION_SUFFIXES.iter().any(|suffix| path.ends_with(suffix))
}

pub fn is_covered(path: &str, covered: &[String], declarations: &[String]) -> bool {
    if declarations.iter().any(|suffix| path.ends_with(suffix)) {
        return true;
    }
    covered.iter().any(|ext| path.ends_with(ext))
}

pub fn classify(path: &str) -> &'static str {
    if path.starts_with("examples/") {
        return CLASS_EXAMPLE;
    }
    if path.contains("/tests/")
        || path.contains("/testdata/")
        || path.contains("/fixtures/")
        || path.starts_with("tests/")
        || path.starts_with("testdata/")
        || path.starts_with("fixtures/")
    {
        return CLASS_TEST;
    }
    let stem = path.rsplit('/').next().unwrap_or(path);
    let stem = stem.rsplit('.').nth(1).unwrap_or(stem);
    if stem == "tests"
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.starts_with("test_")
    {
        return CLASS_TEST;
    }
    if path.ends_with(".pb.go")
        || path.contains("/generated/")
        || stem.contains(".generated.")
    {
        return CLASS_GENERATED;
    }
    CLASS_PROD
}

fn marker_cells(lower: &str) -> Option<Vec<&'static str>> {
    let mut cells: Vec<&'static str> = Vec::new();
    let mut matched = false;
    if lower.contains("windows") {
        matched = true;
        cells.push("windows_x86_64");
        cells.push("windows_arm64");
    }
    if lower.contains("macos") || lower.contains("darwin") || lower.contains("apple") {
        matched = true;
        cells.push("macos_arm64");
    }
    if lower.contains("linux") {
        matched = true;
        cells.push("seed-linux_x86_64");
        cells.push("linux_arm64");
    }
    if lower.contains("aarch64") || lower.contains("arm64") {
        matched = true;
        cells.push("linux_arm64");
        cells.push("macos_arm64");
        cells.push("windows_arm64");
    }
    if lower.contains("x86_64") || lower.contains("amd64") {
        matched = true;
        cells.push("seed-linux_x86_64");
        cells.push("windows_x86_64");
    }
    if lower.contains("wasm") {
        matched = true;
    }
    if !matched {
        return None;
    }
    Some(cells)
}

pub fn cell_applicable(path: &str, cell: &str) -> bool {
    match marker_cells(&path.to_lowercase()) {
        None => true,
        Some(cells) => cells.contains(&cell),
    }
}

pub fn applicable(
    path: &str,
    cell: &str,
    overrides: &BTreeMap<String, Vec<String>>,
) -> bool {
    match overrides.get(path) {
        Some(cells) => cells.iter().any(|named| named == cell),
        None => cell_applicable(path, cell),
    }
}

pub fn parse_applicability(text: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let known: Vec<&str> = CELLS.iter().map(|(cell, _)| *cell).collect();
    let mut rows = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let path = parts.next().unwrap_or_default();
        let cells: Vec<String> = parts.map(str::to_string).collect();
        if path.is_empty() || cells.is_empty() {
            return Err(format!("malformed applicability line {}: {raw:?}", index + 1));
        }
        for cell in &cells {
            if !known.contains(&cell.as_str()) {
                return Err(format!("unknown cell {cell:?} on line {}", index + 1));
            }
        }
        if rows.insert(path.to_string(), cells).is_some() {
            return Err(format!("duplicate applicability path on line {}", index + 1));
        }
    }
    Ok(rows)
}

pub fn seed_measurable(path: &str) -> bool {
    SEED_MEASURED_EXTENSIONS
        .iter()
        .any(|ext| path.ends_with(ext))
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Evidence {
    pub covered: u64,
    pub eligible: u64,
    pub ignored: u64,
}

impl Evidence {
    pub fn uncovered(&self) -> u64 {
        self.eligible.saturating_sub(self.covered)
    }
}

pub fn derive(
    class: &str,
    path: &str,
    applicable: bool,
    measured: Option<&Evidence>,
) -> (&'static str, &'static str) {
    if class == CLASS_TEST {
        return (SUPPORT, REASON_TEST);
    }
    if class == CLASS_EXAMPLE {
        return (SUPPORT, REASON_EXAMPLE);
    }
    if class == CLASS_GENERATED {
        return (SUPPORT, REASON_GENERATED);
    }
    if is_declaration(path) {
        return (SUPPORT, REASON_DECLARATION);
    }
    if !applicable {
        return (SUPPORT, REASON_NOT_APPLICABLE);
    }
    if !seed_measurable(path) {
        return (SUPPORT, REASON_UNMEASURED_LANGUAGE);
    }
    match measured {
        None => (SUPPORT, REASON_ABSENT),
        Some(hits) if hits.eligible == 0 => (SUPPORT, REASON_NO_LINES),
        Some(hits) if hits.uncovered() == 0 => (ELIGIBLE, REASON_FULLY_COVERED),
        Some(_) => (SUPPORT, REASON_UNCOVERED),
    }
}

pub fn normalize(path: &str) -> String {
    let mut out = String::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

pub fn find_aliases(paths: &[String]) -> Vec<(String, String)> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut aliases = Vec::new();
    for path in paths {
        let normal = normalize(path);
        if let Some(first) = seen.get(&normal) {
            if first != path {
                aliases.push((first.clone(), path.clone()));
            }
        } else {
            seen.insert(normal, path.clone());
        }
    }
    aliases
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Denominator {
    pub rows: BTreeMap<String, String>,
}

pub fn parse_denominator(text: &str) -> Result<Denominator, String> {
    let mut rows = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let class = parts.next().unwrap_or_default();
        let path = parts.next().unwrap_or_default();
        if path.is_empty() || parts.next().is_some() {
            return Err(format!("malformed denominator line {}: {raw:?}", index + 1));
        }
        if !matches!(
            class,
            CLASS_PROD | CLASS_TEST | CLASS_EXAMPLE | CLASS_GENERATED
        ) {
            return Err(format!("unknown class {class:?} on line {}", index + 1));
        }
        if rows.insert(path.to_string(), class.to_string()).is_some() {
            return Err(format!("duplicate denominator path on line {}", index + 1));
        }
    }
    Ok(Denominator { rows })
}

pub fn emit_denominator(rows: &BTreeMap<String, String>) -> String {
    let mut out = String::from(
        "# Owned production-source denominator: `<class> <path>` rows, one per tracked covered-language file.\n",
    );
    for (path, class) in rows {
        out.push_str(class);
        out.push(' ');
        out.push_str(path);
        out.push('\n');
    }
    out
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub rows: BTreeMap<String, (String, String)>,
}

pub fn parse_inventory(text: &str) -> Result<Inventory, String> {
    let mut rows = BTreeMap::new();
    let mut reason = String::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix("# reason:") {
            reason = header.trim().to_string();
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let disposition = parts.next().unwrap_or_default();
        let path = parts.next().unwrap_or_default();
        if path.is_empty() || parts.next().is_some() {
            return Err(format!("malformed inventory line {}: {raw:?}", index + 1));
        }
        if disposition != ELIGIBLE && disposition != SUPPORT {
            return Err(format!(
                "unknown disposition {disposition:?} on line {}",
                index + 1
            ));
        }
        if reason.is_empty() {
            return Err(format!("row outside a reason group on line {}", index + 1));
        }
        if !REASONS.contains(&reason.as_str()) {
            return Err(format!("unknown reason {reason:?} on line {}", index + 1));
        }
        if rows
            .insert(
                path.to_string(),
                (disposition.to_string(), reason.clone()),
            )
            .is_some()
        {
            return Err(format!("duplicate inventory path on line {}", index + 1));
        }
    }
    Ok(Inventory { rows })
}

pub fn emit_inventory(rows: &BTreeMap<String, (String, String)>) -> String {
    let mut out = String::from(
        "# Cell exact-gate inventory: `<eligible|support> <path>` rows grouped by `# reason:`.\n",
    );
    let mut by_reason: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (path, (disposition, reason)) in rows {
        by_reason
            .entry(reason.as_str())
            .or_default()
            .push((path.as_str(), disposition.as_str()));
    }
    for reason in REASONS {
        if let Some(group) = by_reason.get(reason) {
            out.push_str("# reason: ");
            out.push_str(reason);
            out.push('\n');
            for (path, disposition) in group {
                out.push_str(disposition);
                out.push(' ');
                out.push_str(path);
                out.push('\n');
            }
        }
    }
    out
}

pub fn parse_measured(text: &str) -> Result<BTreeMap<String, Evidence>, String> {
    let mut rows = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 4 {
            return Err(format!("malformed measured line {}: {raw:?}", index + 1));
        }
        let covered: u64 = parts[1]
            .parse()
            .map_err(|_| format!("bad covered count on line {}", index + 1))?;
        let eligible: u64 = parts[2]
            .parse()
            .map_err(|_| format!("bad eligible count on line {}", index + 1))?;
        let ignored: u64 = parts[3]
            .parse()
            .map_err(|_| format!("bad ignored count on line {}", index + 1))?;
        if covered > eligible {
            return Err(format!("covered exceeds eligible on line {}", index + 1));
        }
        if rows
            .insert(
                parts[0].to_string(),
                Evidence {
                    covered,
                    eligible,
                    ignored,
                },
            )
            .is_some()
        {
            return Err(format!("duplicate measured path on line {}", index + 1));
        }
    }
    Ok(rows)
}

pub fn emit_measured(rows: &BTreeMap<String, Evidence>, header: &[String]) -> String {
    let mut out = String::from(
        "# Measured line evidence: `<path> <covered> <eligible> <ignored>` from real bazel coverage runs.\n",
    );
    for line in header {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    for (path, hits) in rows {
        out.push_str(path);
        out.push_str(&format!(" {} {} {}\n", hits.covered, hits.eligible, hits.ignored));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_routes_owners() {
        assert_eq!(classify("cli/cli/src/main.rs"), CLASS_PROD);
        assert_eq!(classify("gazelle/rust/plugin.go"), CLASS_PROD);
        assert_eq!(classify("examples/adopt-go/greet/greet.go"), CLASS_EXAMPLE);
        assert_eq!(classify("cli/cli/src/exec/managed.rs"), CLASS_PROD);
    }

    #[test]
    fn classify_routes_tests() {
        assert_eq!(classify("cli/lcov/src/verdict_tests.rs"), CLASS_TEST);
        assert_eq!(classify("tools/depcheck/src/tests.rs"), CLASS_TEST);
        assert_eq!(classify("tools/coverage/tests/fixtures/per_cell/pins.bzl"), CLASS_TEST);
        assert_eq!(classify(".devcontainer/devcontainer_parity_test.rs"), CLASS_TEST);
        assert_eq!(classify("go/tests/fixtures/hello/hello_test.go"), CLASS_TEST);
    }

    #[test]
    fn classify_routes_generated() {
        assert_eq!(classify("api/types.pb.go"), CLASS_GENERATED);
        assert_eq!(classify("tools/generated/bindings.rs"), CLASS_GENERATED);
    }

    #[test]
    fn cells_cover_five_platforms() {
        let names: Vec<&str> = CELLS.iter().map(|(cell, _)| *cell).collect();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"windows_arm64"));
        assert_eq!(SEED_CELL, "seed-linux_x86_64");
    }

    #[test]
    fn derive_pins_dispositions() {
        let full = Evidence {
            covered: 40,
            eligible: 40,
            ignored: 0,
        };
        assert_eq!(
            derive(CLASS_PROD, "cli/a.rs", true, Some(&full)),
            (ELIGIBLE, REASON_FULLY_COVERED)
        );
        assert_eq!(
            derive(CLASS_TEST, "cli/a_test.rs", true, Some(&full)),
            (SUPPORT, REASON_TEST)
        );
        assert_eq!(
            derive(CLASS_PROD, "cc/a.cc", true, Some(&full)),
            (SUPPORT, REASON_UNMEASURED_LANGUAGE)
        );
        assert_eq!(
            derive(CLASS_PROD, "cli/a.rs", true, None),
            (SUPPORT, REASON_ABSENT)
        );
        assert_eq!(
            derive(CLASS_PROD, "drv/aarch64_only.rs", false, Some(&full)),
            (SUPPORT, REASON_NOT_APPLICABLE)
        );
        let partial = Evidence {
            covered: 39,
            eligible: 40,
            ignored: 0,
        };
        assert_eq!(
            derive(CLASS_PROD, "cli/a.rs", true, Some(&partial)),
            (SUPPORT, REASON_UNCOVERED)
        );
        let empty = Evidence::default();
        assert_eq!(
            derive(CLASS_PROD, "cli/a.rs", true, Some(&empty)),
            (SUPPORT, REASON_NO_LINES)
        );
    }

    #[test]
    fn derive_models_cell_applicability() {
        let full = Evidence {
            covered: 10,
            eligible: 10,
            ignored: 0,
        };
        let overrides =
            parse_applicability("drv/aarch64_only.rs linux_arm64 macos_arm64 windows_arm64\n")
                .unwrap();
        let arm = applicable("drv/aarch64_only.rs", "windows_arm64", &overrides);
        let seed = applicable("drv/aarch64_only.rs", "seed-linux_x86_64", &overrides);
        let plain = applicable("drv/plain.rs", "windows_arm64", &overrides);
        assert_eq!(
            derive(CLASS_PROD, "drv/aarch64_only.rs", arm, Some(&full)),
            (ELIGIBLE, REASON_FULLY_COVERED)
        );
        assert_eq!(
            derive(CLASS_PROD, "drv/aarch64_only.rs", seed, Some(&full)),
            (SUPPORT, REASON_NOT_APPLICABLE)
        );
        assert_eq!(
            derive(CLASS_PROD, "drv/plain.rs", plain, Some(&full)),
            (ELIGIBLE, REASON_FULLY_COVERED)
        );
        assert!(parse_applicability("drv/a.rs bogus-cell\n").is_err());
    }

    #[test]
    fn normalize_collapses_aliases() {
        assert_eq!(normalize("a//b.rs"), "a/b.rs");
        assert_eq!(normalize("./a/b.rs"), "a/b.rs");
        assert_eq!(normalize("a/./b.rs"), "a/b.rs");
        let aliases = find_aliases(&["a/b.rs".to_string(), "a//b.rs".to_string()]);
        assert_eq!(aliases.len(), 1);
        let clean = find_aliases(&["a/b.rs".to_string(), "a/c.rs".to_string()]);
        assert!(clean.is_empty());
    }

    #[test]
    fn inventory_roundtrip_keeps_reason_groups() {
        let mut rows = BTreeMap::new();
        rows.insert(
            "b.rs".to_string(),
            (SUPPORT.to_string(), REASON_TEST.to_string()),
        );
        rows.insert(
            "a.rs".to_string(),
            (ELIGIBLE.to_string(), REASON_FULLY_COVERED.to_string()),
        );
        let text = emit_inventory(&rows);
        let parsed = parse_inventory(&text).unwrap();
        assert_eq!(parsed.rows, rows);
    }

    #[test]
    fn extract_covered_reads_the_owner_table() {
        let text = "pub(crate) const COVERED_LANGUAGES: &[(&str, LineComment)] = &[\n    (\".rs\", LineComment::SlashSlash),\n    (\".h\", LineComment::SlashSlash),\n];\nconst DECLARATION_SUFFIXES: &[&str] = &[\"../x\", \".d.ts\"];\nfn is_starlark(p: &str) -> bool {\n    p.ends_with(\".bzl\")\n}\n";
        let (covered, declarations) = extract_covered(text).unwrap();
        assert!(covered.contains(&".rs".to_string()));
        assert!(covered.contains(&".h".to_string()));
        assert!(!covered.contains(&".bzl".to_string()));
        assert_eq!(declarations, vec![".d.ts".to_string()]);
        assert!(extract_covered("no tables here\n").is_err());
    }

    #[test]
    fn inventory_rejects_rows_outside_reason_groups() {        assert!(parse_inventory("eligible a.rs\n").is_err());
        assert!(parse_inventory("# reason: bogus\nsupport a.rs\n").is_err());
        assert!(parse_inventory("# reason: test-source\nsupport a.rs\nsupport a.rs\n").is_err());
    }

    #[test]
    fn denominator_rejects_unknown_classes_and_duplicates() {
        assert!(parse_denominator("bogus a.rs\n").is_err());
        assert!(parse_denominator("prod a.rs\nprod a.rs\n").is_err());
        let parsed = parse_denominator("# head\nprod a.rs\ntest b.rs\n").unwrap();
        assert_eq!(parsed.rows.len(), 2);
    }
}
