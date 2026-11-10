//! Command line entry for the Lua docs converter.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::process::ExitCode;

fn main() -> ExitCode {
    documentation_ir::tool::main(
        "lua_extract",
        dx_docs_lua_extract::usage(),
        dx_docs_lua_extract::parse,
        dx_docs_lua_extract::run,
    )
}
