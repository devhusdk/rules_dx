"""Wrapper conformance tests for Dart."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "DART_SOURCE_SPECS", "dart_cross_target_error", "dart_deferred_tags", "dart_entry_sources", "dart_root_package_name")

def dart_wrapper_contract_tests(name):
    """Instantiates Dart wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = [file_class for file_class, _ in DART_SOURCE_SPECS],
        extra_checks = [
            expect_equal("dart rules own dart sources", DART_SOURCE_SPECS, [("dart", "dart")]),
            expect_equal("entrypoint comes first", dart_entry_sources("bin/main.dart", ["bin/flags.dart"]), ["bin/main.dart", "bin/flags.dart"]),
            expect_equal("missing srcs keep the entrypoint", dart_entry_sources("bin/main.dart", None), ["bin/main.dart"]),
            expect_equal("entrypoint listed twice is owned once", dart_entry_sources("bin/main.dart", ["bin/main.dart"]), ["bin/main.dart"]),
            expect_equal("root library takes its own name", dart_root_package_name("", "tally", {}), "tally"),
            expect_equal("nested library keeps the directory name", dart_root_package_name("pkgs/tally", "lib", {}), None),
            expect_equal("explicit root package name wins", dart_root_package_name("", "lib", {"package_name": "tally"}), None),
            expect_equal("native sdk is accepted", dart_cross_target_error("", ""), ""),
            expect_equal("cross sdk is refused", dart_cross_target_error("linux", "riscv64"), "dart cross-compilation to linux-riscv64 is not supported: dart compile downloads the target snapshotter at action time"),
            expect_equal("root package metadata wins", dart_root_package_name("", "lib", {"package": ":meta"}), None),
            expect_equal("deferred tags keep quality aspects off", dart_deferred_tags(["x"]), ["x", "no-format", "no-lint", "no-typecheck"]),
        ],
    )
