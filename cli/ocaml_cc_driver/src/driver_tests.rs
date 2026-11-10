use crate::driver::*;
use std::ffi::OsString;

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

fn words(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn names_every_tool_by_file_stem() {
    for tool in TOOLS {
        assert_eq!(tool_of(&format!("x/bin/conda-ocaml-{tool}")).unwrap(), tool);
        assert_eq!(tool_of(&format!("conda-ocaml-{tool}.exe")).unwrap(), tool);
    }
}

#[test]
fn rejects_other_names() {
    assert!(
        tool_of("conda-ocaml-strip")
            .unwrap_err()
            .contains("is not one of")
    );
    assert!(tool_of("ocaml_cc_driver").is_err());
    assert!(tool_of("").is_err());
}

#[test]
fn assembler_keeps_its_leading_flags() {
    let planned = plan(
        "as",
        &env(&[("CONDA_OCAML_AS", "x/clang -c"), ("DX_OCAML_LIB", "d/lib")]),
        words(&["-o", "a.o", "a.s"]),
    )
    .unwrap();
    assert_eq!(planned.program, "x/clang");
    assert_eq!(planned.args, words(&["-c", "-o", "a.o", "a.s"]));
}

#[test]
fn compiler_and_linkers_search_the_library_directory() {
    for (tool, variable) in [
        ("cc", "CONDA_OCAML_CC"),
        ("mkdll", "CONDA_OCAML_MKDLL"),
        ("mkexe", "CONDA_OCAML_MKEXE"),
    ] {
        let lookup = move |name: &str| match name {
            "DX_OCAML_LIB" => Some("d/lib".to_owned()),
            other if other == variable => Some("x/clang -shared".to_owned()),
            _ => None,
        };
        let planned = plan(tool, &lookup, words(&["a.o"])).unwrap();
        assert_eq!(planned.program, "x/clang");
        assert_eq!(planned.args, words(&["-shared", "-Ld/lib", "a.o"]));
    }
}

#[test]
fn linker_runs_without_a_library_directory() {
    let planned = plan(
        "mkexe",
        &env(&[("CONDA_OCAML_MKEXE", "x/clang++")]),
        words(&["a.o"]),
    )
    .unwrap();
    assert_eq!(planned.args, words(&["a.o"]));
}

#[test]
fn unset_or_blank_tool_fails_and_names_the_variable() {
    let error = plan("ranlib", &env(&[]), Vec::new()).unwrap_err();
    assert_eq!(error, "conda-ocaml-ranlib: CONDA_OCAML_RANLIB is not set");
    assert!(plan("cc", &env(&[("CONDA_OCAML_CC", "  ")]), Vec::new()).is_err());
}

#[test]
fn archiver_uses_its_variable() {
    let planned = plan(
        "ar",
        &env(&[("CONDA_OCAML_AR", "x/llvm-ar")]),
        words(&["rc", "lib.a", "a.o"]),
    )
    .unwrap();
    assert_eq!(planned.program, "x/llvm-ar");
    assert_eq!(planned.args, words(&["rc", "lib.a", "a.o"]));
}

#[test]
fn archiver_translates_to_libtool() {
    let both = env(&[
        ("CONDA_OCAML_AR", "x/ar"),
        ("DX_OCAML_LIBTOOL", "x/libtool"),
    ]);
    let planned = plan("ar", &both, words(&["rc", "lib.a", "a.o", "b.o"])).unwrap();
    assert_eq!(planned.program, "x/libtool");
    assert_eq!(
        planned.args,
        words(&["-static", "-o", "lib.a", "a.o", "b.o"])
    );
    assert!(
        plan("ar", &both, words(&["rc"]))
            .unwrap_err()
            .contains("want MODE OUT")
    );
}

#[test]
fn as_drops_the_debug_prefix_map_pair_when_asked() {
    let planned = plan(
        "as",
        &env(&[
            ("CONDA_OCAML_AS", "clang -c"),
            ("DX_OCAML_AS_DROP_DEBUG_PREFIX_MAP", "1"),
        ]),
        words(&["--debug-prefix-map", "a=b", "-o", "x.o"]),
    )
    .unwrap();
    assert_eq!(planned.args, words(&["-c", "-o", "x.o"]));
}

#[test]
fn as_keeps_the_debug_prefix_map_otherwise() {
    let planned = plan(
        "as",
        &env(&[("CONDA_OCAML_AS", "as")]),
        words(&["--debug-prefix-map", "a=b"]),
    )
    .unwrap();
    assert_eq!(planned.args, words(&["--debug-prefix-map", "a=b"]));
}
