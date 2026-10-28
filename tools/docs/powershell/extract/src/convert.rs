//! Maps the exported functions onto the versioned documentation IR.

use crate::exports::{public_functions, wildcard_match, Public};
use crate::types::{parse_api, Api, Function, Names, Parameter, ParameterSet, SCHEMA};
use documentation_ir::proto::{
    DocIr, Extension, Param, Returns, SourceRef, Symbol, SymbolKind, Visibility,
};
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};

const LANGUAGE: &str = "powershell";
const KEY_ALIASES: &str = "powershell.aliases";
const KEY_CONFIG: &str = "powershell.config";
const KEY_INCOMPLETE: &str = "powershell.incomplete";
const ALL_SETS: &str = "__AllParameterSets";
const DEFAULT_TYPE: &str = "Object";

/// The identity inputs of one extraction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub package: String,
    pub config: String,
}

/// Converts one dx.powershell.api/1 document into a validated IR shard.
pub fn convert(json: &str, settings: &Settings) -> Result<DocIr, String> {
    let api = parse_api(json)?;
    let public = public_functions(&api)?;
    let config = config_text(&api, settings);
    let mut symbols: Vec<Symbol> = public
        .iter()
        .map(|public| symbol(&api, public, settings, &config))
        .collect();
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: LANGUAGE.to_owned(),
        package: settings.package.clone(),
        symbols,
    };
    documentation_ir::validate_shard(&shard).map_err(|error| format!("invalid IR: {error:?}"))?;
    Ok(shard)
}

fn config_text(api: &Api, settings: &Settings) -> String {
    let manifest = api.manifest.as_ref();
    let mut text = format!(
        "manifest={};module_version={};pwsh={};api={SCHEMA}",
        manifest.map(|m| m.rel.as_str()).unwrap_or_default(),
        manifest
            .and_then(|m| m.module_version.as_deref())
            .unwrap_or_default(),
        api.pwsh
    );
    if !settings.config.is_empty() {
        text.push_str(&format!(";config={}", settings.config));
    }
    text
}

fn symbol(api: &Api, public: &Public<'_>, settings: &Settings, config: &str) -> Symbol {
    let function = public.function;
    let help = function.help.as_ref();
    let doc = help
        .map(|help| {
            [clean(&help.synopsis), clean(&help.description)]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default();
    let params = function
        .params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            r#type: type_name(param).to_owned(),
            doc: param_doc(function, &param.name),
        })
        .collect();
    let outputs: Vec<String> = help
        .map(|help| {
            help.outputs
                .iter()
                .map(|o| clean(o))
                .filter(|o| !o.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let returns = if function.output_types.is_empty() && outputs.is_empty() {
        None
    } else {
        Some(Returns {
            r#type: function.output_types.join(", "),
            doc: outputs.join("\n\n"),
        })
    };
    let examples = help
        .map(|help| {
            help.examples
                .iter()
                .map(|e| clean(e))
                .filter(|e| !e.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let mut extensions = Vec::new();
    let aliases = exported_aliases(api, function);
    if !aliases.is_empty() {
        extensions.push(Extension {
            key: KEY_ALIASES.to_owned(),
            value: aliases.join(",").into_bytes(),
        });
    }
    extensions.push(Extension {
        key: KEY_CONFIG.to_owned(),
        value: config.as_bytes().to_vec(),
    });
    if let Some(missing) = incomplete(function) {
        extensions.push(Extension {
            key: KEY_INCOMPLETE.to_owned(),
            value: missing.into_bytes(),
        });
    }
    extensions.sort_by(|a, b| a.key.cmp(&b.key));
    Symbol {
        id: format!("{LANGUAGE}:{}:{}", settings.package, function.name),
        kind: SymbolKind::Function as i32,
        signature_text: signature(function),
        doc_markdown: doc,
        params,
        returns,
        examples,
        source: Some(SourceRef {
            file: public.file.rel.clone(),
            line: function.line,
            generated: false,
        }),
        visibility: Visibility::Public as i32,
        relations: None,
        extensions,
        ..Symbol::default()
    }
}

/// Normalizes help text: LF line ends and no surrounding blank space.
pub fn clean(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_owned()
}

fn type_name(param: &Parameter) -> &str {
    param
        .type_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(DEFAULT_TYPE)
}

fn is_switch(param: &Parameter) -> bool {
    matches!(
        type_name(param).to_ascii_lowercase().as_str(),
        "switch" | "switchparameter" | "system.management.automation.switchparameter"
    )
}

fn param_doc(function: &Function, name: &str) -> String {
    function
        .help
        .as_ref()
        .and_then(|help| {
            help.parameters
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
        })
        .map(|(_, doc)| clean(doc))
        .unwrap_or_default()
}

fn exported_aliases(api: &Api, function: &Function) -> Vec<String> {
    let filter = api
        .manifest
        .as_ref()
        .and_then(|m| m.aliases_to_export.as_ref());
    let mut aliases: Vec<String> = function
        .aliases
        .iter()
        .filter(|alias| match filter {
            None | Some(Names::All(_)) => true,
            Some(Names::List(patterns)) => patterns.iter().any(|p| wildcard_match(p, alias)),
        })
        .cloned()
        .collect();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn incomplete(function: &Function) -> Option<String> {
    let Some(help) = &function.help else {
        return Some("help".to_owned());
    };
    let mut missing = Vec::new();
    if clean(&help.synopsis).is_empty() {
        missing.push("synopsis".to_owned());
    }
    let undocumented: Vec<&str> = function
        .params
        .iter()
        .filter(|param| param_doc(function, &param.name).is_empty())
        .map(|param| param.name.as_str())
        .collect();
    if !undocumented.is_empty() {
        missing.push(format!("params={}", undocumented.join(",")));
    }
    if missing.is_empty() {
        None
    } else {
        Some(missing.join(";"))
    }
}

fn is_advanced(function: &Function) -> bool {
    function.cmdlet_binding.is_some() || function.params.iter().any(|p| !p.sets.is_empty())
}

fn set_names(function: &Function) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Some(default) = function
        .cmdlet_binding
        .as_ref()
        .and_then(|b| b.default_set.as_ref())
    {
        if default != ALL_SETS {
            names.push(default.clone());
        }
    }
    for param in &function.params {
        for set in &param.sets {
            if set.name != ALL_SETS && !names.contains(&set.name) {
                names.push(set.name.clone());
            }
        }
    }
    if names.is_empty() {
        names.push(ALL_SETS.to_owned());
    }
    names
}

fn binding(param: &Parameter, set: &str) -> Option<ParameterSet> {
    if param.sets.is_empty() {
        return Some(ParameterSet {
            name: ALL_SETS.to_owned(),
            mandatory: false,
            position: None,
            pipeline: false,
        });
    }
    param
        .sets
        .iter()
        .find(|s| s.name == set)
        .or_else(|| param.sets.iter().find(|s| s.name == ALL_SETS))
        .cloned()
}

fn implicit_positions(function: &Function) -> Vec<Option<u32>> {
    let positional_binding = function
        .cmdlet_binding
        .as_ref()
        .is_none_or(|b| b.positional_binding);
    let explicit = function
        .params
        .iter()
        .any(|p| p.sets.iter().any(|s| s.position.is_some()));
    let mut next = 0;
    function
        .params
        .iter()
        .map(|param| {
            if !positional_binding || explicit || is_switch(param) {
                return None;
            }
            let position = next;
            next += 1;
            Some(position)
        })
        .collect()
}

/// Renders one Get-Help style syntax line per parameter set, default set first.
pub fn signature(function: &Function) -> String {
    let implicit = implicit_positions(function);
    let common = if is_advanced(function) {
        " [<CommonParameters>]"
    } else {
        ""
    };
    set_names(function)
        .iter()
        .map(|set| {
            let mut positional: Vec<(u32, String)> = Vec::new();
            let mut named: Vec<String> = Vec::new();
            for (index, param) in function.params.iter().enumerate() {
                let Some(bound) = binding(param, set) else {
                    continue;
                };
                let position = bound.position.or(implicit[index]);
                let text = param_text(param, bound.mandatory, position.is_some());
                match position {
                    Some(position) => positional.push((position, text)),
                    None => named.push(text),
                }
            }
            positional.sort_by_key(|(position, _)| *position);
            let mut line = function.name.clone();
            for part in positional.into_iter().map(|(_, text)| text).chain(named) {
                line.push(' ');
                line.push_str(&part);
            }
            line.push_str(common);
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn param_text(param: &Parameter, mandatory: bool, positional: bool) -> String {
    let core = if is_switch(param) {
        format!("-{}", param.name)
    } else if positional {
        format!("[-{}] <{}>", param.name, type_name(param))
    } else {
        format!("-{} <{}>", param.name, type_name(param))
    };
    if mandatory {
        core
    } else {
        format!("[{core}]")
    }
}
