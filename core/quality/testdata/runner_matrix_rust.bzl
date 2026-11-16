"""Matrix Rust cases."""

load(":runner_matrix_tests.bzl", "matrix_case")

def _rust_case(name, edition = "2021", **kwargs):
    return matrix_case(
        name = name,
        capability = "format",
        tool = "rustfmt",
        source_class = "rust",
        tags = ["no-remote-exec"],
        rustfmt_from_toolchain = True,
        edition_tools = ["rustfmt"],
        edition_values = [edition],
        **kwargs
    )

_EDITION_GATE = "pub fn edition_gate() -> i32 {\n    let async = 1;\n    async\n}\n"

RUST_CASES = [
    _rust_case("matrix_rust_format_pass", src = ":clean.rs"),
    _rust_case(
        "matrix_rust_format_fail",
        generated = {
            "matrix/rustfmt_dirty.rs": "pub fn add(a:i32,b:i32)->i32 {\n    a+b\n}\n",
        },
    ),
    _rust_case(
        "matrix_rust_format_edition_2015",
        edition = "2015",
        platforms = ["linux_x86_64"],
        generated = {"matrix/rustfmt_edition_2015.rs": _EDITION_GATE},
    ),
    _rust_case(
        "matrix_rust_format_edition_mismatch",
        platforms = ["linux_x86_64"],
        generated = {"matrix/rustfmt_edition_mismatch.rs": _EDITION_GATE},
    ),
    _rust_case(
        "matrix_rust_format_modules",
        platforms = ["linux_x86_64"],
        generated = {
            "matrix/rustfmt_modules/src/lib.rs": "mod child;\nmod nested;\n#[path = \"renamed/impl.rs\"]\nmod renamed;\npub fn root( ){ }\n",
            "matrix/rustfmt_modules/src/child.rs": "pub fn child( ) {}\n",
            "matrix/rustfmt_modules/src/nested.rs": "mod grand;\n",
            "matrix/rustfmt_modules/src/nested/grand.rs": "pub fn grand( ) {}\n",
            "matrix/rustfmt_modules/src/renamed/impl.rs": "pub fn renamed( ) {}\n",
        },
        path = "matrix/rustfmt_modules/src/lib.rs,matrix/rustfmt_modules/src/child.rs,matrix/rustfmt_modules/src/nested.rs,matrix/rustfmt_modules/src/nested/grand.rs,matrix/rustfmt_modules/src/renamed/impl.rs",
    ),
    _rust_case(
        "matrix_rust_format_supporting_module",
        platforms = ["linux_x86_64"],
        generated = {
            "core/quality/testdata/matrix/rustfmt_supporting/src/lib.rs": "mod child;\npub fn root( ){ }\n",
        },
        siblings = [
            ":matrix/rustfmt_supporting/src/child.rs",
            ":matrix/rustfmt_supporting/src/child/grand.rs",
        ],
    ),
    _rust_case(
        "matrix_rust_format_missing_module",
        platforms = ["linux_x86_64"],
        generated = {
            "matrix/rustfmt_missing/src/lib.rs": "mod child;\npub fn root( ){ }\n",
        },
    ),
]
