use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, SymbolSpec};
use crate::types::AdapterError;
use crate::CPP_DOXYGEN_PIN;

pub fn normalize_cpp(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    if !input.contains("<doxygen") {
        return Err(AdapterError::InvalidJson("missing doxygen root".to_owned()));
    }
    if !input.contains(&format!("version=\"{CPP_DOXYGEN_PIN}\""))
        && !input.contains(&format!("version='{CPP_DOXYGEN_PIN}'"))
        && !input.contains(CPP_DOXYGEN_PIN)
    {
        return Err(AdapterError::VersionMismatch {
            expected: CPP_DOXYGEN_PIN.to_owned(),
            found: "unpinned".to_owned(),
        });
    }
    let mut symbols = Vec::new();
    for member in split_members(input) {
        let kind = tag_attr(&member, "memberdef", "kind").unwrap_or_else(|| "function".to_owned());
        let name = tag_text(&member, "name").unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let brief = tag_text(&member, "briefdescription").unwrap_or_default();
        let file = location_file(&member).unwrap_or_default();
        let line = location_line(&member).unwrap_or(1);
        symbols.push(make_symbol(SymbolSpec {
            language: "cpp",
            package,
            qualified: &name,
            kind_name: &kind,
            signature: format!("{kind} {name}"),
            doc: brief,
            file,
            line,
        })?);
    }
    shard("cpp", package, symbols)
}

fn split_members(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<memberdef") {
        let tail = &rest[start..];
        if let Some(end) = tail.find("</memberdef>") {
            out.push(tail[..end + "</memberdef>".len()].to_owned());
            rest = &tail[end + "</memberdef>".len()..];
        } else {
            break;
        }
    }
    out
}

fn tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = block.find(&open)?;
    let content_start = block[start..].find('>')? + start + 1;
    let end = block[content_start..].find(&close)? + content_start;
    Some(block[content_start..end].trim().to_owned())
}

fn tag_attr(block: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = block.find(&open)?;
    let end = block[start..].find('>')? + start;
    let header = &block[start..end];
    let needle = format!("{attr}=\"");
    let attr_start = header.find(&needle)? + needle.len();
    let attr_end = header[attr_start..].find('"')? + attr_start;
    Some(header[attr_start..attr_end].to_owned())
}

fn location_file(block: &str) -> Option<String> {
    let start = block.find("<location")?;
    let end = block[start..].find('>')? + start;
    tag_attr(&block[start..end + 1], "location", "file")
}

fn location_line(block: &str) -> Option<u64> {
    let start = block.find("<location")?;
    let end = block[start..].find('>')? + start;
    tag_attr(&block[start..end + 1], "location", "line")?
        .parse()
        .ok()
}
