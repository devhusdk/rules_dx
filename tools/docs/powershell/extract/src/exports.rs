use crate::types::{Api, ExportKind, File, Function, Names};
use crate::{Duplicate, PowershellExtractError as E};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Public<'a> {
    pub file: &'a File,
    pub function: &'a Function,
}

pub fn public_functions(api: &Api) -> Result<Vec<Public<'_>>, E> {
    if let Some(manifest) = &api.manifest
        && let Some(root) = &manifest.root_module
        && root.to_ascii_lowercase().ends_with(".dll")
    {
        return Err(E::BinaryRootModule {
            rel: manifest.rel.clone(),
            root: root.clone(),
        });
    }
    let all: Vec<Public<'_>> = api
        .files
        .iter()
        .flat_map(|file| {
            file.functions
                .iter()
                .map(move |function| Public { file, function })
        })
        .collect();
    let defined: Vec<Public<'_>> = all
        .iter()
        .filter(|public| in_module_scope(public.function))
        .copied()
        .collect();
    let mut chosen = exported_by_module(api, &all, &defined)?;
    if let Some(manifest) = &api.manifest {
        match &manifest.functions_to_export {
            None => {}
            Some(Names::All(all)) if all == "*" => {}
            Some(Names::All(other)) => {
                return Err(E::BadFunctionsToExport {
                    rel: manifest.rel.clone(),
                    value: other.clone(),
                });
            }
            Some(Names::List(patterns)) => {
                require_defined(&manifest.rel, patterns, &all)?;
                chosen.retain(|public| {
                    patterns
                        .iter()
                        .any(|pattern| wildcard_match(pattern, &public.function.name))
                });
            }
        }
    }
    if chosen.is_empty() {
        return Err(E::NoExports);
    }
    reject_case_duplicates(&chosen)?;
    Ok(chosen)
}

fn exported_by_module<'a>(
    api: &'a Api,
    all: &[Public<'a>],
    defined: &[Public<'a>],
) -> Result<Vec<Public<'a>>, E> {
    let mut patterns: Vec<&str> = Vec::new();
    let mut literal = false;
    for file in &api.files {
        match file.exports.kind {
            ExportKind::None => {}
            ExportKind::Dynamic => {
                return Err(E::DynamicExport {
                    rel: file.rel.clone(),
                });
            }
            ExportKind::Literal => {
                literal = true;
                require_defined(&file.rel, &file.exports.functions, all)?;
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

fn in_module_scope(function: &Function) -> bool {
    !matches!(function.scope.as_deref(), Some("global" | "private"))
}

fn require_defined(rel: &str, patterns: &[String], defined: &[Public<'_>]) -> Result<(), E> {
    for pattern in patterns {
        if has_wildcard(pattern) {
            continue;
        }
        if !defined
            .iter()
            .any(|public| wildcard_match(pattern, &public.function.name))
        {
            return Err(E::UndefinedExport {
                rel: rel.to_owned(),
                pattern: pattern.clone(),
            });
        }
    }
    Ok(())
}

fn reject_case_duplicates(chosen: &[Public<'_>]) -> Result<(), E> {
    let mut seen: BTreeMap<String, &Public<'_>> = BTreeMap::new();
    for public in chosen {
        let key = public.function.name.to_lowercase();
        if let Some(first) = seen.get(&key) {
            let reason = if first.function.name == public.function.name {
                "is defined twice"
            } else {
                "differs only by case from"
            };
            return Err(E::DuplicateFunction(Box::new(Duplicate {
                rel: public.file.rel.clone(),
                line: public.function.line,
                name: public.function.name.clone(),
                reason,
                first: first.function.name.clone(),
                first_rel: first.file.rel.clone(),
                first_line: first.function.line,
            })));
        }
        seen.insert(key, public);
    }
    Ok(())
}

fn has_wildcard(pattern: &str) -> bool {
    pattern.contains(['*', '?'])
}

#[must_use]
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
            Some(&c) if c == '?' || name.get(n) == Some(&c) => {
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
    pattern
        .get(p..)
        .is_some_and(|tail| tail.iter().all(|&c| c == '*'))
}
