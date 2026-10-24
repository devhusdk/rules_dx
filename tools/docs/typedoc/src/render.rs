//! Renders documentation IR shards as one Markdown API reference page.

use documentation_ir::proto::{DocIr, Symbol};
use std::collections::BTreeMap;

/// The presentation inputs of one rendered reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub title: String,
    pub source_url: String,
}

/// Renders every shard's symbols as one Markdown page.
pub fn render(shards: &[DocIr], options: &Options) -> Result<String, String> {
    if !options.source_url.is_empty() && !options.source_url.contains("{file}") {
        return Err("--source-url must contain {file}".to_owned());
    }
    let mut symbols: BTreeMap<&str, (&DocIr, &Symbol)> = BTreeMap::new();
    for shard in shards {
        for symbol in &shard.symbols {
            if symbols.insert(&symbol.id, (shard, symbol)).is_some() {
                return Err(format!("duplicate symbol id '{}'", symbol.id));
            }
        }
    }
    if symbols.is_empty() {
        return Err("no symbols to render".to_owned());
    }
    let mut out = format!("# {}\n\n", options.title);
    for (id, (_, symbol)) in &symbols {
        let depth = depth(symbol, &symbols);
        out.push_str(&format!(
            "{}- [{}](#{id})\n",
            "  ".repeat(depth.saturating_sub(2)),
            display(id)
        ));
    }
    out.push('\n');
    for (id, (_, symbol)) in &symbols {
        out.push_str(&section(id, symbol, &symbols, options)?);
    }
    Ok(out)
}

fn display(id: &str) -> &str {
    id.splitn(3, ':').nth(2).unwrap_or(id)
}

fn depth(symbol: &Symbol, symbols: &BTreeMap<&str, (&DocIr, &Symbol)>) -> usize {
    let mut level = 2;
    let mut current = symbol;
    while let Some(parent) = current
        .relations
        .as_ref()
        .and_then(|relations| relations.member_of.first())
    {
        let Some((_, found)) = symbols.get(parent.as_str()) else {
            break;
        };
        level += 1;
        current = found;
        if level >= 6 {
            break;
        }
    }
    level.min(6)
}

fn language_of(shard: &DocIr) -> &str {
    match shard.language.as_str() {
        "typescript" => "ts",
        other => other,
    }
}

fn section(
    id: &str,
    symbol: &Symbol,
    symbols: &BTreeMap<&str, (&DocIr, &Symbol)>,
    options: &Options,
) -> Result<String, String> {
    let (shard, _) = symbols[id];
    let level = depth(symbol, symbols);
    let mut out = format!(
        "<a id=\"{id}\"></a>\n{} {}\n\n",
        "#".repeat(level),
        display(id)
    );
    out.push_str(&format!(
        "```{}\n{}\n```\n\n",
        language_of(shard),
        symbol.signature_text
    ));
    if !symbol.doc_markdown.is_empty() {
        out.push_str(&symbol.doc_markdown);
        out.push_str("\n\n");
    }
    if !symbol.params.is_empty() {
        out.push_str("**Parameters**\n\n");
        for param in &symbol.params {
            out.push_str(&format!("- `{}`: `{}`", param.name, param.r#type));
            if !param.doc.is_empty() {
                out.push_str(&format!(". {}", param.doc));
            }
            out.push('\n');
        }
        out.push('\n');
    }
    if let Some(returns) = symbol.returns.as_ref() {
        out.push_str(&format!("**Returns**: `{}`", returns.r#type));
        if !returns.doc.is_empty() {
            out.push_str(&format!(". {}", returns.doc));
        }
        out.push_str("\n\n");
    }
    for example in &symbol.examples {
        out.push_str("**Example**\n\n");
        if example.trim_start().starts_with("```") {
            out.push_str(example);
        } else {
            out.push_str(&format!("```{}\n{example}\n```", language_of(shard)));
        }
        out.push_str("\n\n");
    }
    if let Some(relations) = symbol.relations.as_ref() {
        for parent in &relations.member_of {
            if symbols.contains_key(parent.as_str()) {
                out.push_str(&format!("Member of [{}](#{parent})\n\n", display(parent)));
            }
        }
        if !relations.implements.is_empty() {
            let linked: Vec<String> = relations
                .implements
                .iter()
                .map(|name| {
                    let target = format!("{}:{}:{name}", shard.language, shard.package);
                    if symbols.contains_key(target.as_str()) {
                        format!("[{name}](#{target})")
                    } else {
                        format!("`{name}`")
                    }
                })
                .collect();
            out.push_str(&format!("Implements {}\n\n", linked.join(", ")));
        }
    }
    for extension in &symbol.extensions {
        if extension.key == "typescript.reexport_of" {
            let target = String::from_utf8_lossy(&extension.value).into_owned();
            if symbols.contains_key(target.as_str()) {
                out.push_str(&format!(
                    "Re-export of [{}](#{target})\n\n",
                    display(&target)
                ));
            } else {
                return Err(format!("'{id}' re-exports unknown symbol '{target}'"));
            }
        }
    }
    if let Some(source) = symbol.source.as_ref() {
        let label = format!("{}:{}", source.file, source.line);
        if options.source_url.is_empty() {
            out.push_str(&format!("Source: `{label}`\n\n"));
        } else {
            let url = options
                .source_url
                .replace("{file}", &source.file)
                .replace("{line}", &source.line.to_string());
            out.push_str(&format!("Source: [{label}]({url})\n\n"));
        }
    }
    Ok(out)
}
