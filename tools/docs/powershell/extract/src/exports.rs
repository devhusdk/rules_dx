//! Resolves which functions a module exports.

use crate::types::{Api, ExportKind, File, Function, Names};
use std::collections::BTreeMap;

/// One exported function and the file that defines it.
#[derive(Clone, Copy, Debug)]
pub struct Public<'a> {
    pub file: &'a File,
    pub function: &'a Function,
}

/// Returns the exported functions in definition order, or why the set cannot be known.
pub fn public_functions(api: &Api) -> Result<Vec<Public<'_>>, String> {
    if let Some(manifest) = &api.manifest {
        if let Some(root) = &manifest.root_module {
            if root.to_ascii_lowercase().ends_with(".dll") {
                return Err(format!(
                    "{}: RootModule '{root}' is a binary module, which is not supported",
                    manifest.rel
                ));
            }
        }
    }
    let defined: Vec<Public<'_>> = api
        .files
        .iter()
        .flat_map(|file| {
            file.functions
                .iter()
                .map(move |function| Public { file, function })
        })
        .collect();
    let mut chosen = exported_by_module(api, &defined)?;
    if let Some(manifest) = &api.manifest {
        match &manifest.functions_to_export {
            None => {}
            Some(Names::All(all)) if all == "*" => {}
            Some(Names::All(other)) => {
                return Err(format!(
                    "{}: FunctionsToExport '{other}' must be '*' or a list",
                    manifest.rel
                ))
            }
            Some(Names::List(patterns)) => {
                require_defined(&manifest.rel, patterns, &defined)?;
                chosen.retain(|public| {
                    patterns
                        .iter()
                        .any(|pattern| wildcard_match(pattern, &public.function.name))
                });
            }
        }
    }
    if chosen.is_empty() {
        return Err("the module exports no functions".to_owned());
    }
    reject_case_duplicates(&chosen)?;
    Ok(chosen)
}

fn exported_by_module<'a>(api: &'a Api, defined: &[Public<'a>]) -> Result<Vec<Public<'a>>, String> {
    let mut patterns: Vec<&str> = Vec::new();
    let mut literal = false;
    for file in &api.files {
        match file.exports.kind {
            ExportKind::None => {}
            ExportKind::Dynamic => {
                return Err(format!(
                    "{}: Export-ModuleMember -Function is not a literal list",
                    file.rel
                ))
            }
            ExportKind::Literal => {
                literal = true;
                require_defined(&file.rel, &file.exports.functions, defined)?;
                patterns.extend(file.exports.functions.iter().map(String::as_str));
            }
        }
    }
    if !literal {
        return Ok(defined.to_vec());
    }
    Ok(defined
        .iter()
        .filter(|public| {
            patterns
                .iter()
                .any(|pattern| wildcard_match(pattern, &public.function.name))
        })
        .copied()
        .collect())
}

fn require_defined(rel: &str, patterns: &[String], defined: &[Public<'_>]) -> Result<(), String> {
    for pattern in patterns {
        if has_wildcard(pattern) {
            continue;
        }
        if !defined
            .iter()
            .any(|public| wildcard_match(pattern, &public.function.name))
        {
            return Err(format!(
                "{rel}: exports '{pattern}' but no source file defines it"
            ));
        }
    }
    Ok(())
}

fn reject_case_duplicates(chosen: &[Public<'_>]) -> Result<(), String> {
    let mut seen: BTreeMap<String, &Public<'_>> = BTreeMap::new();
    for public in chosen {
        let key = public.function.name.to_lowercase();
        if let Some(first) = seen.get(&key) {
            let reason = if first.function.name == public.function.name {
                "is defined twice"
            } else {
                "differs only by case from"
            };
            return Err(format!(
                "{}:{}: function '{}' {reason} '{}' at {}:{}",
                public.file.rel,
                public.function.line,
                public.function.name,
                first.function.name,
                first.file.rel,
                first.function.line
            ));
        }
        seen.insert(key, public);
    }
    Ok(())
}

fn has_wildcard(pattern: &str) -> bool {
    pattern.contains(['*', '?'])
}

/// Matches one PowerShell wildcard (`*` and `?`) against a name, ignoring case.
pub fn wildcard_match(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let name: Vec<char> = name.to_lowercase().chars().collect();
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, n));
                p += 1;
            }
            Some(&c) if c == '?' || c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                Some((star_p, star_n)) => {
                    p = star_p + 1;
                    n = star_n + 1;
                    star = Some((star_p, star_n + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}
