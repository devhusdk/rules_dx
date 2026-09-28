"""Layer-2 matrix cases (snapshot workflow): every supported language x capability cell."""

RUST_CASES = [
    {
        "name": "matrix_rust_format_pass",
        "srcs": [":clean.rs"],
        "capability": "format",
        "stages": ["rustfmt;rust;quality/testdata/clean.rs"],
        "rustfmt_from_toolchain": True,
        "edition_tools": ["rustfmt"],
        "edition_values": ["2021"],
        "expected_file": ":matrix/matrix_rust_format_pass.expected.txt",
    },
    {
        "name": "matrix_rust_format_fail",
        "generated": {
            "matrix/rustfmt_dirty.rs": "pub fn add(a:i32,b:i32)->i32 {\n    a+b\n}\n",
        },
        "capability": "format",
        "stages": ["rustfmt;rust;matrix/rustfmt_dirty.rs"],
        "rustfmt_from_toolchain": True,
        "edition_tools": ["rustfmt"],
        "edition_values": ["2021"],
        "expected_file": ":matrix/matrix_rust_format_fail.expected.txt",
    },
    {
        "name": "matrix_rust_format_edition_2015",
        "generated": {
            "matrix/rustfmt_edition_2015.rs": "pub fn edition_gate() -> i32 {\n    let async = 1;\n    async\n}\n",
        },
        "capability": "format",
        "stages": ["rustfmt;rust;matrix/rustfmt_edition_2015.rs"],
        "rustfmt_from_toolchain": True,
        "edition_tools": ["rustfmt"],
        "edition_values": ["2015"],
        "expected_file": ":matrix/matrix_rust_format_edition_2015.expected.txt",
    },
    {
        "name": "matrix_rust_format_edition_mismatch",
        "generated": {
            "matrix/rustfmt_edition_mismatch.rs": "pub fn edition_gate() -> i32 {\n    let async = 1;\n    async\n}\n",
        },
        "capability": "format",
        "stages": ["rustfmt;rust;matrix/rustfmt_edition_mismatch.rs"],
        "rustfmt_from_toolchain": True,
        "edition_tools": ["rustfmt"],
        "edition_values": ["2021"],
        "expected_file": ":matrix/matrix_rust_format_edition_mismatch.expected.txt",
    },
    {
        "name": "matrix_rust_lint_pass",
        "generated": {
            "matrix/clippy_clean.rs": "fn main() {}\n",
        },
        "capability": "lint",
        "stages": ["clippy;rust;matrix/clippy_clean.rs"],
        "upstream_tools": ["clippy"],
        "upstream_srcs": [":matrix/matrix_rust_lint_pass.upstream.clippy.txt"],
        "expected_file": ":matrix/matrix_rust_lint_pass.expected.txt",
    },
    {
        "name": "matrix_rust_lint_fail",
        "generated": {
            "matrix/clippy_len.rs": "fn f(x:u8){\n       \"x\".len() == 0\n}\n",
        },
        "capability": "lint",
        "stages": ["clippy;rust;matrix/clippy_len.rs"],
        "upstream_tools": ["clippy"],
        "upstream_srcs": [":matrix/matrix_rust_lint_fail.upstream.clippy.txt"],
        "expected_file": ":matrix/matrix_rust_lint_fail.expected.txt",
    },
    {
        "name": "matrix_rust_typecheck_pass",
        "generated": {
            "matrix/rustc_type_clean.rs": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        },
        "capability": "typecheck",
        "stages": ["rustc;rust;matrix/rustc_type_clean.rs"],
        "upstream_tools": ["rustc"],
        "upstream_srcs": [":matrix/matrix_rust_typecheck_pass.upstream.rustc.txt"],
        "expected_file": ":matrix/matrix_rust_typecheck_pass.expected.txt",
    },
    {
        "name": "matrix_rust_typecheck_fail",
        "generated": {
            "matrix/rustc_type.rs": "pub fn add(a: i32, b: i32) -> i32 {\n    a + \"two\"\n}\n",
        },
        "capability": "typecheck",
        "stages": ["rustc;rust;matrix/rustc_type.rs"],
        "upstream_tools": ["rustc"],
        "upstream_srcs": [":matrix/matrix_rust_typecheck_fail.upstream.rustc.txt"],
        "expected_file": ":matrix/matrix_rust_typecheck_fail.expected.txt",
    },
]
