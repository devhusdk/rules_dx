/// Keeps the records whose path is a listed source or ends in `/<source>`, renamed to the longest such source.
pub fn keep_sources(lcov: &str, sources: &[&str]) -> String {
    let mut out = String::new();
    let mut keep = false;
    for line in lcov.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let found = sources.iter().find(|src| path == **src).or_else(|| {
                sources
                    .iter()
                    .filter(|src| path.ends_with(&format!("/{src}")))
                    .max_by_key(|src| src.len())
            });
            keep = found.is_some();
            if let Some(src) = found {
                out.push_str(&format!("SF:{src}\n"));
            }
            continue;
        }
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
#[path = "keep_tests.rs"]
mod keep_tests;
