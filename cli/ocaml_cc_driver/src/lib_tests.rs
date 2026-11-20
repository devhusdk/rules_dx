use super::*;

#[test]
fn other_program_names_fail() {
    assert_eq!(run_tool(OsStr::new("ocaml_cc_driver"), Vec::new()), 1);
}

#[test]
fn dune_without_a_mode_fails() {
    assert_eq!(run_dune(&[]), 1);
}
