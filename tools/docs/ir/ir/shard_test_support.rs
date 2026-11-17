//! Helpers of the tests that read documentation shards.

use documentation_ir::decode_shard;
use documentation_ir::proto::{DocIr, Symbol};

/// Decodes the shard that the environment variable `var` names.
///
/// # Errors
///
/// Fails when the shard cannot be read or decoded.
pub fn shard(var: &str) -> Result<DocIr, String> {
    let path = dx_testing::env_file(var);
    let bytes =
        std::fs::read(&path).map_err(|error| format!("read the shard {}: {error}", path.display()))?;
    decode_shard(&bytes).map_err(|error| format!("decode the shard {}: {error:?}", path.display()))
}

/// Finds a symbol by id.
#[must_use]
pub fn find<'a>(shard: &'a DocIr, id: &str) -> Option<&'a Symbol> {
    shard.symbols.iter().find(|s| s.id == id)
}

/// Finds a symbol by id.
///
/// # Errors
///
/// Fails when the symbol is missing.
pub fn symbol<'a>(shard: &'a DocIr, id: &str) -> Result<&'a Symbol, String> {
    find(shard, id).ok_or_else(|| format!("{id} is missing"))
}

/// Returns the ids of all symbols.
#[must_use]
pub fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

/// Returns the value of the extension `key` of `symbol`.
///
/// # Errors
///
/// Fails when the extension is not UTF-8.
pub fn extension<'a>(symbol: &'a Symbol, key: &str) -> Result<Option<&'a str>, String> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .map(|e| std::str::from_utf8(&e.value).map_err(|error| format!("extension {key}: {error}")))
        .transpose()
}

/// Returns the 1-based line of the first fixture line that contains `needle`.
///
/// # Errors
///
/// Fails when the fixture cannot be read or does not contain `needle`.
pub fn line_of(var: &str, needle: &str) -> Result<u64, String> {
    let path = dx_testing::env_file(var);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("read the fixture {}: {error}", path.display()))?;
    let index = text
        .lines()
        .position(|line| line.contains(needle))
        .ok_or_else(|| format!("{needle} is not in the fixture"))?;
    Ok(index as u64 + 1)
}
