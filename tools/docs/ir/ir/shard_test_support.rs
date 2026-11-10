//! Helpers of the tests that read documentation shards.

use documentation_ir::decode_shard;
use documentation_ir::proto::{DocIr, Symbol};

/// Decodes the shard that the environment variable `var` names.
pub fn shard(var: &str) -> DocIr {
    let bytes = std::fs::read(dx_testing::env_file(var)).expect("read the shard");
    decode_shard(&bytes).expect("the shard decodes")
}

/// Finds a symbol by id.
pub fn find<'a>(shard: &'a DocIr, id: &str) -> Option<&'a Symbol> {
    shard.symbols.iter().find(|s| s.id == id)
}

/// Finds a symbol by id and panics when it is missing.
pub fn symbol<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    find(shard, id).unwrap_or_else(|| panic!("{id} is missing"))
}

/// Returns the ids of all symbols.
pub fn ids(shard: &DocIr) -> Vec<&str> {
    shard.symbols.iter().map(|s| s.id.as_str()).collect()
}

/// Returns the value of the extension `key` of `symbol`.
pub fn extension<'a>(symbol: &'a Symbol, key: &str) -> Option<&'a str> {
    symbol
        .extensions
        .iter()
        .find(|e| e.key == key)
        .map(|e| std::str::from_utf8(&e.value).expect("utf-8 extension"))
}

/// Returns the 1-based line of the first fixture line that contains `needle`.
pub fn line_of(var: &str, needle: &str) -> u64 {
    let text = std::fs::read_to_string(dx_testing::env_file(var)).expect("read the fixture");
    let index = text
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle} is not in the fixture"));
    index as u64 + 1
}
