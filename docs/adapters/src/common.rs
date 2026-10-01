use documentation_ir::proto::{DocIr, Relations, SourceRef, Symbol, SymbolKind, Visibility};
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};

use crate::types::AdapterError;

pub struct SymbolSpec<'a> {
    pub language: &'a str,
    pub package: &'a str,
    pub qualified: &'a str,
    pub kind_name: &'a str,
    pub signature: String,
    pub doc: String,
    pub file: String,
    pub line: u64,
}

pub fn shard(language: &str, package: &str, symbols: Vec<Symbol>) -> Result<DocIr, AdapterError> {
    if language.is_empty() || package.is_empty() {
        return Err(AdapterError::EmptyIdentity);
    }
    let mut ordered = symbols;
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    let mut previous: Option<&str> = None;
    for symbol in &ordered {
        if let Some(prev) = previous {
            if prev == symbol.id {
                return Err(AdapterError::DuplicateId(symbol.id.clone()));
            }
        }
        previous = Some(symbol.id.as_str());
        if let Some(source) = symbol.source.as_ref() {
            if let Some(reason) = documentation_ir::source_path_reason(&source.file) {
                return Err(AdapterError::UnsafePath {
                    id: symbol.id.clone(),
                    path: source.file.clone(),
                    reason,
                });
            }
        }
    }
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: language.to_owned(),
        package: package.to_owned(),
        symbols: ordered,
    };
    documentation_ir::validate_shard(&shard)
        .map_err(|err| AdapterError::InvalidIr(format!("{err:?}")))?;
    Ok(shard)
}

pub fn symbol_id(language: &str, package: &str, qualified: &str) -> Result<String, AdapterError> {
    if language.is_empty() || package.is_empty() || qualified.is_empty() {
        return Err(AdapterError::EmptyIdentity);
    }
    Ok(format!("{language}:{package}:{qualified}"))
}

pub fn overload_id(base: &str, params: &[String]) -> String {
    let normalized: Vec<&str> = params
        .iter()
        .map(|ty| ty.trim())
        .filter(|ty| !ty.is_empty())
        .collect();
    format!("{}({})", base, normalized.join(","))
}

pub fn text(value: &serde_json::Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

pub fn kind_of(name: &str) -> i32 {
    match name {
        "module" => SymbolKind::Module as i32,
        "class" | "struct" | "interface" => SymbolKind::Class as i32,
        "function" => SymbolKind::Function as i32,
        "method" => SymbolKind::Method as i32,
        "field" | "prop" | "property" => SymbolKind::Property as i32,
        "constant" | "const" => SymbolKind::Constant as i32,
        "enum" => SymbolKind::Enum as i32,
        "variant" => SymbolKind::EnumVariant as i32,
        "alias" | "type" => SymbolKind::TypeAlias as i32,
        _ => SymbolKind::Function as i32,
    }
}

pub fn make_symbol(spec: SymbolSpec<'_>) -> Result<Symbol, AdapterError> {
    let SymbolSpec {
        language,
        package,
        qualified,
        kind_name,
        signature,
        doc,
        file,
        line,
    } = spec;
    let id = symbol_id(language, package, qualified)?;
    Ok(Symbol {
        id,
        kind: kind_of(kind_name),
        signature_text: signature,
        doc_markdown: doc,
        params: vec![],
        returns: None,
        examples: vec![],
        source: Some(SourceRef { file, line }),
        visibility: Visibility::Public as i32,
        relations: Some(Relations {
            member_of: vec![],
            implements: vec![],
        }),
        extensions: vec![],
    })
}

pub fn encode_ir(shard: &DocIr) -> Result<Vec<u8>, AdapterError> {
    documentation_ir::encode_shard(shard).map_err(|err| AdapterError::InvalidIr(format!("{err:?}")))
}
