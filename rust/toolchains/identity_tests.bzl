"""Identity tests pinning the authoritative toolchain binding."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":bindings.bzl", "RUSTFMT_TOOLCHAIN_TYPE", "RUST_TOOLCHAIN_TYPE")

_AT = "@"

EXPECTED_IDENTITY_OBSERVATIONS = """subject //rust/toolchains:identity_under_test
field clippy.owner=""" + _AT + """@rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain
field clippy.path=bazel-out/k8-fastbuild/bin/external/rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools/rust_toolchain/bin/clippy-driver
field rustc.owner=""" + _AT + """@rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain
field rustc.path=bazel-out/k8-fastbuild/bin/external/rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools/rust_toolchain/bin/rustc
field rustfmt.owner=""" + _AT + """@rules_rust++rust+rustfmt_1.98.1__x86_64-unknown-linux-gnu_tools//:bin/rustfmt
field rustfmt.path=external/rules_rust++rust+rustfmt_1.98.1__x86_64-unknown-linux-gnu_tools/bin/rustfmt
aspect_field aspect_seen=True
aspect_field field_count=6
aspect_field has_subject=True
aspect_field subject_label=//rust/toolchains:identity_under_test
aspect_field transitive_count=0"""

_LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def toolchain_identity_tests(name):
    starlark_test(
        name = name + "_binding",
        mode = "load",
        checks = [
            expect_equal(
                "clippy binds to the main Rust toolchain type",
                RUST_TOOLCHAIN_TYPE,
                "@rules_rust//rust:toolchain_type",
            ),
            expect_equal(
                "rustfmt binds to the registered rustfmt toolchain type",
                RUSTFMT_TOOLCHAIN_TYPE,
                "@rules_rust//rust/rustfmt:toolchain_type",
            ),
        ],
    )
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":identity_under_test"],
        expected_observations = EXPECTED_IDENTITY_OBSERVATIONS,
        target_compatible_with = _LINUX_X86_64,
    )
