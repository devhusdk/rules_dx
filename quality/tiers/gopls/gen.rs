use std::collections::BTreeMap;
use std::fs;

fn quoted_names(path: &str) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    text.lines()
        .filter_map(|line| line.trim().strip_prefix('"'))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_owned)
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [govet, staticcheck, govet_all, staticcheck_all] = args.as_slice() else {
        panic!("usage: gen <govet tier> <staticcheck tier> <govet strict> <staticcheck strict>");
    };
    let mut analyses = BTreeMap::new();
    for name in quoted_names(govet_all)
        .into_iter()
        .chain(quoted_names(staticcheck_all))
    {
        analyses.insert(name, false);
    }
    for name in quoted_names(govet)
        .into_iter()
        .chain(quoted_names(staticcheck))
    {
        analyses.insert(name, true);
    }
    println!("{{");
    println!("  \"ui.diagnostic.staticcheck\": true,");
    println!("  \"ui.diagnostic.analyses\": {{");
    let last = analyses.len() - 1;
    for (i, (name, on)) in analyses.iter().enumerate() {
        let comma = if i == last { "" } else { "," };
        println!("    \"{name}\": {on}{comma}");
    }
    println!("  }}");
    println!("}}");
}
