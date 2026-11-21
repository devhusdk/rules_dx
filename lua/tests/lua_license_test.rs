//! The shipped Lua and LuaFileSystem licence texts match the pinned upstream sources.

use dx_testing::{env_file, read_text};

/// The copyright block at the end of `lua.h`, without its `/****` and `****/` rules and the leading `* ` of each line.
fn copyright_block(header: &str) -> String {
    let header = header.replace("\r\n", "\n");
    let lines: Vec<&str> = header.lines().collect();
    let start = lines
        .windows(2)
        .position(|pair| pair[0].starts_with("/*****") && pair[1].contains("Copyright (C)"))
        .expect("lua.h has a copyright block");
    let mut block = String::new();
    for line in &lines[start + 1..] {
        if line.starts_with("*****") {
            return block;
        }
        let text = line
            .strip_prefix("* ")
            .or_else(|| (*line == "*").then_some(""))
            .unwrap_or_else(|| panic!("unexpected line in the lua.h copyright block: {line:?}"));
        block.push_str(text);
        block.push('\n');
    }
    panic!("the lua.h copyright block is not closed");
}

#[test]
fn lua_licence_matches_lua_h() {
    let expected = copyright_block(&read_text(&env_file("LUA_H")));
    let shipped = read_text(&env_file("LUA_LICENSE")).replace("\r\n", "\n");
    assert_eq!(shipped, expected);
}

#[test]
fn luafilesystem_exports_its_licence() {
    let licence = read_text(&env_file("LUAFILESYSTEM_LICENSE"));
    assert!(
        licence.contains("Permission is hereby granted"),
        "{licence}"
    );
}
