use std::ffi::OsStr;

use dx_cli::multicall::subcommand_for;

#[test]
fn every_hub_subcommand_is_a_dx_subcommand() {
    let key = dx_testing::env_text("DX_HUB_TARGETS");
    let path = dx_testing::rlocation(key.trim()).expect("dx_hub_targets.txt is a runfile");
    let text = dx_testing::read_text(&path);
    let names: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    assert!(!names.is_empty(), "dx_hub_targets.txt names no subcommand");
    let unknown: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| subcommand_for(OsStr::new(name)).is_none())
        .collect();
    assert_eq!(unknown, Vec::<&str>::new(), "dx runs no such subcommand");
}
