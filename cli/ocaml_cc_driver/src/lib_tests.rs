use super::*;

#[test]
fn driver_names_are_the_conda_ocaml_programs() {
    assert!(is_driver(OsStr::new("conda-ocaml-cc")));
    assert!(is_driver(OsStr::new("bin/conda-ocaml-mkexe.exe")));
    assert!(!is_driver(OsStr::new("conda-ocaml-ocamlopt")));
    assert!(!is_driver(OsStr::new("ocaml_dune")));
    assert!(!is_driver(OsStr::new("dx")));
}

#[test]
fn other_program_names_fail() {
    assert_eq!(run_tool(OsStr::new("ocaml_cc_driver"), Vec::new()), 1);
}

#[test]
fn dune_without_a_mode_fails() {
    assert_eq!(run_dune(&[]), 1);
}
