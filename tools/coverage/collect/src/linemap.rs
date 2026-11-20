//! Files coverage of nvcc's kept host translation unit under the sources its linemarkers name.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const GENERATED_SUFFIXES: [&str; 2] = [".cudafe1.cpp", ".cudafe1.stub.c"];

/// Maps lines of a preprocessed file to `(source, line)` from its `# N "f"` and `#line N "f"` markers.
pub struct LineMap {
    lines: Vec<Option<(String, u64)>>,
}

impl LineMap {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut current: Option<(String, u64)> = None;
        for line in text.lines() {
            if let Some((number, file)) = marker(line) {
                let file = file.or_else(|| current.as_ref().map(|(f, _)| f.clone()));
                current = file.map(|f| (f, number));
                lines.push(None);
                continue;
            }
            lines.push(current.clone());
            if let Some((_, ref mut n)) = current {
                *n += 1;
            }
        }
        Self { lines }
    }

    /// The source position of 1-based `line`, if a marker covers it.
    #[must_use]
    pub fn lookup(&self, line: u64) -> Option<(&str, u64)> {
        let index = usize::try_from(line.checked_sub(1)?).ok()?;
        let (file, n) = self.lines.get(index)?.as_ref()?;
        Some((file.as_str(), *n))
    }
}

fn marker(line: &str) -> Option<(u64, Option<String>)> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let rest = rest.strip_prefix("line").unwrap_or(rest);
    if !rest.starts_with([' ', '\t']) && !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let rest = rest.trim_start();
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (number, rest) = rest.split_at_checked(digits)?;
    let number = number.parse().ok()?;
    let rest = rest.trim_start();
    let Some(quoted) = rest.strip_prefix('"') else {
        return rest.is_empty().then_some((number, None));
    };
    let end = quoted.find('"')?;
    let (file, _) = quoted.split_at_checked(end)?;
    Some((number, Some(file.to_string())))
}

fn kept_target(file: &str) -> Option<String> {
    let file = file.replace('\\', "/");
    let file = file.strip_prefix("./").unwrap_or(&file);
    let dropped = file.starts_with('<')
        || file.starts_with('/')
        || file.get(1..2) == Some(":")
        || file.starts_with("external/")
        || file.contains("/external/");
    (!dropped).then(|| file.to_string())
}

#[derive(Default)]
struct Record {
    preamble: Vec<String>,
    functions: BTreeMap<String, u64>,
    function_hits: BTreeMap<String, u64>,
    lines: BTreeMap<u64, u64>,
    branches: BTreeMap<(u64, String, String), Option<u64>>,
}

impl Record {
    fn parse(body: &[&str]) -> Self {
        let mut record = Self::default();
        for line in body {
            if let Some(rest) = line.strip_prefix("FN:") {
                if let Some((n, name)) = rest.split_once(',')
                    && let Ok(n) = n.parse()
                {
                    record.functions.insert(name.to_string(), n);
                }
            } else if let Some(rest) = line.strip_prefix("FNDA:") {
                if let Some((count, name)) = rest.split_once(',')
                    && let Ok(count) = count.parse()
                {
                    record.hit_function(name, count);
                }
            } else if let Some(rest) = line.strip_prefix("DA:") {
                let mut parts = rest.split(',');
                if let (Some(Ok(n)), Some(Ok(count))) =
                    (parts.next().map(str::parse), parts.next().map(str::parse))
                {
                    record.hit_line(n, count);
                }
            } else if let Some(rest) = line.strip_prefix("BRDA:") {
                let parts: Vec<&str> = rest.splitn(4, ',').collect();
                if let [n, block, branch, taken] = parts.as_slice()
                    && let Ok(n) = n.parse()
                {
                    record.take_branch(
                        (n, (*block).to_string(), (*branch).to_string()),
                        taken.parse().ok(),
                    );
                }
            } else if !is_total(line) {
                record.preamble.push((*line).to_string());
            }
        }
        record
    }

    fn hit_function(&mut self, name: &str, count: u64) {
        let hits = self.function_hits.entry(name.to_string()).or_default();
        *hits = (*hits).max(count);
    }

    fn hit_line(&mut self, line: u64, count: u64) {
        let hits = self.lines.entry(line).or_default();
        *hits = (*hits).max(count);
    }

    fn take_branch(&mut self, key: (u64, String, String), taken: Option<u64>) {
        let slot = self.branches.entry(key).or_default();
        *slot = match (*slot, taken) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }

    fn render(&self, out: &mut String, source: &str) {
        for line in &self.preamble {
            writeln!(out, "{line}").unwrap_or_default();
        }
        writeln!(out, "SF:{source}").unwrap_or_default();
        for (name, line) in &self.functions {
            writeln!(out, "FN:{line},{name}").unwrap_or_default();
        }
        for (name, count) in &self.function_hits {
            writeln!(out, "FNDA:{count},{name}").unwrap_or_default();
        }
        writeln!(out, "FNF:{}", self.functions.len()).unwrap_or_default();
        writeln!(
            out,
            "FNH:{}",
            self.function_hits.values().filter(|&&c| c > 0).count()
        )
        .unwrap_or_default();
        for ((line, block, branch), taken) in &self.branches {
            let taken = taken.map_or_else(|| "-".to_string(), |t| t.to_string());
            writeln!(out, "BRDA:{line},{block},{branch},{taken}").unwrap_or_default();
        }
        writeln!(out, "BRF:{}", self.branches.len()).unwrap_or_default();
        writeln!(
            out,
            "BRH:{}",
            self.branches
                .values()
                .filter(|t| t.is_some_and(|t| t > 0))
                .count()
        )
        .unwrap_or_default();
        for (line, count) in &self.lines {
            writeln!(out, "DA:{line},{count}").unwrap_or_default();
        }
        writeln!(out, "LF:{}", self.lines.len()).unwrap_or_default();
        writeln!(
            out,
            "LH:{}",
            self.lines.values().filter(|&&c| c > 0).count()
        )
        .unwrap_or_default();
        writeln!(out, "end_of_record").unwrap_or_default();
    }

    /// Adds this generated file's counts to the targets its markers name.
    fn remap_into(
        &self,
        map: &LineMap,
        targets: &mut BTreeMap<String, Self>,
        order: &mut Vec<String>,
    ) {
        let mut target = |file: &str| -> Option<String> {
            let file = kept_target(file)?;
            if !targets.contains_key(&file) {
                order.push(file.clone());
                targets.insert(
                    file.clone(),
                    Self {
                        preamble: self.preamble.clone(),
                        ..Self::default()
                    },
                );
            }
            Some(file)
        };
        let mut placed = Vec::new();
        for (&line, &count) in &self.lines {
            if let Some((file, n)) = map.lookup(line)
                && let Some(file) = target(file)
            {
                placed.push((file, Placed::Line(n, count)));
            }
        }
        for (name, &line) in &self.functions {
            if let Some((file, n)) = map.lookup(line)
                && let Some(file) = target(file)
            {
                let count = self.function_hits.get(name).copied().unwrap_or(0);
                placed.push((file, Placed::Function(n, name.clone(), count)));
            }
        }
        for ((line, block, branch), &taken) in &self.branches {
            if let Some((file, n)) = map.lookup(*line)
                && let Some(file) = target(file)
            {
                placed.push((
                    file,
                    Placed::Branch((n, block.clone(), branch.clone()), taken),
                ));
            }
        }
        for (file, item) in placed {
            let Some(record) = targets.get_mut(&file) else {
                continue;
            };
            match item {
                Placed::Line(n, count) => record.hit_line(n, count),
                Placed::Function(n, name, count) => {
                    record.functions.entry(name.clone()).or_insert(n);
                    record.hit_function(&name, count);
                }
                Placed::Branch(key, taken) => record.take_branch(key, taken),
            }
        }
    }
}

enum Placed {
    Line(u64, u64),
    Function(u64, String, u64),
    Branch((u64, String, String), Option<u64>),
}

fn is_total(line: &str) -> bool {
    ["FNF:", "FNH:", "BRF:", "BRH:", "LF:", "LH:"]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

/// Re-files every `SF:` record of an nvcc host file (`*.cudafe1.cpp`, `*.cudafe1.stub.c`) under the
/// sources its linemarkers name, merging into existing records by taking the higher count. `locate`
/// finds the kept file for an `SF:` path; a record whose file it cannot find is left as is.
#[must_use]
pub fn remap(report: &str, locate: &dyn Fn(&str) -> Option<PathBuf>) -> String {
    let mut out = String::new();
    let mut records: BTreeMap<String, Record> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut generated: Vec<(Record, LineMap)> = Vec::new();
    let mut body: Vec<&str> = Vec::new();
    let mut source: Option<&str> = None;
    let mut changed = false;
    for line in report.lines() {
        if let Some(sf) = line.strip_prefix("SF:") {
            source = Some(sf.trim_end());
        } else if line == "end_of_record" {
            let record = Record::parse(&body);
            body.clear();
            let Some(sf) = source.take() else {
                continue;
            };
            let map = GENERATED_SUFFIXES
                .iter()
                .any(|suffix| sf.ends_with(suffix))
                .then(|| locate(sf))
                .flatten()
                .and_then(|path| fs::read_to_string(path).ok())
                .map(|text| LineMap::parse(&text));
            if let Some(map) = map {
                generated.push((record, map));
                changed = true;
            } else if let Some(existing) = records.get_mut(sf) {
                existing.merge(record);
            } else {
                order.push(sf.to_string());
                records.insert(sf.to_string(), record);
            }
        } else {
            body.push(line);
        }
    }
    if !changed {
        return report.to_string();
    }
    for (record, map) in &generated {
        record.remap_into(map, &mut records, &mut order);
    }
    for sf in &order {
        if let Some(record) = records.get(sf) {
            record.render(&mut out, sf);
        }
    }
    out
}

impl Record {
    fn merge(&mut self, other: Self) {
        for (name, line) in other.functions {
            self.functions.entry(name).or_insert(line);
        }
        for (name, count) in other.function_hits {
            self.hit_function(&name, count);
        }
        for (line, count) in other.lines {
            self.hit_line(line, count);
        }
        for (key, taken) in other.branches {
            self.take_branch(key, taken);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const KEPT: &str = "# 1 \"cuda/tests/conformance/greet.cu\"\n\
        int a;\n\
        # 1 \"/usr/include/stdio.h\" 1 3\n\
        int s;\n\
        # 3 \"cuda/tests/conformance/greet.cu\" 2\n\
        int f() {\n\
        return 1;\n\
        #line 10 \"cuda/tests/conformance/greet.h\"\n\
        int g;\n\
        # 1 \"<built-in>\"\n\
        int b;\n";

    fn scratch(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("linemap_{name}_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("greet.compute_120.cudafe1.cpp");
        fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn parses_markers() {
        let map = LineMap::parse(KEPT);
        assert_eq!(map.lookup(1), None);
        assert_eq!(map.lookup(2), Some(("cuda/tests/conformance/greet.cu", 1)));
        assert_eq!(map.lookup(4), Some(("/usr/include/stdio.h", 1)));
        assert_eq!(map.lookup(6), Some(("cuda/tests/conformance/greet.cu", 3)));
        assert_eq!(map.lookup(7), Some(("cuda/tests/conformance/greet.cu", 4)));
        assert_eq!(map.lookup(9), Some(("cuda/tests/conformance/greet.h", 10)));
        assert_eq!(map.lookup(11), Some(("<built-in>", 1)));
        assert_eq!(map.lookup(0), None);
        assert_eq!(map.lookup(99), None);
    }

    #[test]
    fn reads_marker_forms() {
        assert_eq!(
            marker("# 7 \"a.cu\" 2"),
            Some((7, Some("a.cu".to_string())))
        );
        assert_eq!(
            marker("#line 7 \"a.cu\""),
            Some((7, Some("a.cu".to_string())))
        );
        assert_eq!(marker("#line 7"), Some((7, None)));
        assert_eq!(marker("#include \"a.h\""), None);
        assert_eq!(marker("#pragma once"), None);
        assert_eq!(marker("int x;"), None);
    }

    #[test]
    fn merges_lines_functions_and_branches_into_targets() {
        let kept = scratch("merge", KEPT);
        let sf = "bazel-out/k8/bin/greet.keep/greet.compute_120.cudafe1.cpp";
        let report = format!(
            "SF:cuda/tests/conformance/greet.cu\nDA:3,5\nDA:4,0\nLF:2\nLH:2\nend_of_record\n\
             SF:{sf}\nFN:6,_Z1fv\nFNDA:7,_Z1fv\nFNF:1\nFNH:1\nBRDA:7,0,0,3\nBRDA:7,0,1,-\nBRF:2\nBRH:1\n\
             DA:2,1\nDA:4,9\nDA:6,2\nDA:7,4\nDA:9,8\nDA:11,6\nLF:6\nLH:6\nend_of_record\n"
        );
        let out = remap(&report, &|path| (path == sf).then(|| kept.clone()));
        assert_eq!(
            out,
            "SF:cuda/tests/conformance/greet.cu\nFN:3,_Z1fv\nFNDA:7,_Z1fv\nFNF:1\nFNH:1\n\
             BRDA:4,0,0,3\nBRDA:4,0,1,-\nBRF:2\nBRH:1\n\
             DA:1,1\nDA:3,5\nDA:4,4\nLF:3\nLH:3\nend_of_record\n\
             SF:cuda/tests/conformance/greet.h\nFNF:0\nFNH:0\nBRF:0\nBRH:0\nDA:10,8\nLF:1\nLH:1\nend_of_record\n"
        );
        fs::remove_dir_all(kept.parent().unwrap()).unwrap();
    }

    #[test]
    fn drops_builtin_absolute_and_external_targets() {
        assert_eq!(kept_target("<built-in>"), None);
        assert_eq!(kept_target("<command-line>"), None);
        assert_eq!(kept_target("/usr/include/stdio.h"), None);
        assert_eq!(kept_target("C:\\cuda\\include\\x.h"), None);
        assert_eq!(kept_target("external/cuda/include/x.h"), None);
        assert_eq!(kept_target("./a/b.cu"), Some("a/b.cu".to_string()));
    }

    #[test]
    fn leaves_other_records_untouched() {
        let report = "TN:\nSF:a.cc\nDA:1,1\nLF:1\nLH:1\nend_of_record\n";
        assert_eq!(
            remap(report, &|_| panic!("located a non-nvcc file")),
            report
        );
    }

    #[test]
    fn keeps_the_record_when_the_kept_file_is_missing() {
        let report = "SF:x.cudafe1.cpp\nDA:1,1\nLF:1\nLH:1\nend_of_record\n";
        assert_eq!(remap(report, &|_| None), report);
        let missing = Path::new("/nonexistent/x.cudafe1.cpp").to_path_buf();
        assert_eq!(remap(report, &|_| Some(missing.clone())), report);
    }
}
