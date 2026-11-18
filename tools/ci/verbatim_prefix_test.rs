//! Only dx_path spells a Windows verbatim prefix; other Rust code calls `dx_path::without_verbatim` and its siblings.

use dx_guard::lexer::{Token, rust_tokens};
use dx_guard::snapshot::snapshot;
use std::path::Path;
use yaml_serde as _;

/// The string literals a hand-written strip or add of a verbatim prefix spells.
/// A path that only starts with a prefix, like a test's `r"\\?\C:\a"`, is not one of them.
const PREFIXES: [&str; 6] = [r"\\?\", r"\\?\UNC\", "//?/", "//?/UNC/", r"UNC\", "UNC/"];

/// The directories that may spell the prefixes.
const OWNERS: [&str; 2] = [
    // The one shared helper.
    "cli/path/",
    // The MSVC linker wrapper every dx Rust crate links through, so it cannot depend on dx_path.
    "rust/private/tools/msvc_paths/",
];

/// The verbatim-prefix literals in one Rust source.
fn prefixes_in(source: &str) -> Vec<String> {
    rust_tokens(source)
        .into_iter()
        .filter_map(|token| match token {
            Token::Text(text) if PREFIXES.contains(&text.as_str()) => Some(text),
            Token::Text(_) | Token::Word(_) | Token::Punct(_) => None,
        })
        .collect()
}

#[test]
fn only_dx_path_spells_a_verbatim_prefix() {
    let found: Vec<String> = snapshot()
        .iter()
        .filter(|(path, _)| {
            Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
                && !OWNERS.iter().any(|owner| path.starts_with(owner))
        })
        .flat_map(|(path, source)| {
            prefixes_in(source)
                .into_iter()
                .map(move |text| format!("{path}: {text:?}"))
        })
        .collect();
    assert!(
        found.is_empty(),
        "spell no verbatim prefix outside dx_path; call dx_path::without_verbatim, \
         without_verbatim_str, without_verbatim_in or verbatim instead:\n{}",
        found.join("\n")
    );
}

#[test]
fn every_spelling_of_a_prefix_is_found() {
    for source in [
        r#"let a = text.strip_prefix(r"\\?\");"#,
        r#"const P: &str = "\\\\?\\";"#,
        r##"let a = text.strip_prefix(r#"\\?\UNC\"#);"##,
        r#"let a = text.replace("//?/", "");"#,
        r#"let a = text.strip_prefix("//?/UNC/");"#,
        r#"let a = rest.starts_with("UNC\\");"#,
    ] {
        assert_eq!(prefixes_in(source).len(), 1, "{source}");
    }
}

#[test]
fn paths_that_start_with_a_prefix_and_comments_are_not_found() {
    for source in [
        r#"let p = Path::new(r"\\?\C:\a");"#,
        r#"let p = Path::new("\\\\?\\UNC\\h\\s");"#,
        "// drops the `\\\\?\\` prefix\nfn f() {}",
        r#"let p = "UNC";"#,
    ] {
        assert!(prefixes_in(source).is_empty(), "{source}");
    }
}
