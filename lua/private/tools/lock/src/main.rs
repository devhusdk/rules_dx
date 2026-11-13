//! The lock step of the luarocks ecosystem.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::process::ExitCode;

use lua_lock::{BASE, Lua, Site};

fn main() -> ExitCode {
    let Some(hub) = dx_families::hub("luarocks") else {
        eprintln!("luarocks has no registry entry");
        return ExitCode::from(2);
    };
    let spec = Lua::spec(hub);
    dx_lock::main(&spec, &Lua::new(&spec, BASE, &Site::new()))
}
