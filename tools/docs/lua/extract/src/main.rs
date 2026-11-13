//! Command line entry for the Lua docs converter.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "lua_extract",
        dx_docs_lua_extract::usage(),
        dx_docs_lua_extract::parse,
        dx_docs_lua_extract::run,
    )
}
