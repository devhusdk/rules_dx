"""Unit tests for the Dart staging helpers."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":dart_context.bzl", "DART_ANALYZE", "DART_FORMAT", "dart_package_entries", "dart_pubspec_stub", "dart_pubspec_stubs")

def _pkg(name, lib_root, language_version = ""):
    return struct(package_name = name, lib_root = lib_root, language_version = language_version)

_PACKAGES = [
    _pkg("app", "", "3.4"),
    _pkg("lib_a", "pkgs/a"),
    _pkg("collection", "../dx_dart_deps+collection", "3.4"),
    _pkg("sub", "../repo/sub"),
]

def dart_context_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("format lists in-repo packages only", [e["name"] for e in dart_package_entries(_PACKAGES, DART_FORMAT)], ["app", "lib_a"]),
            expect_equal("format entry", dart_package_entries(_PACKAGES, DART_FORMAT)[0], {"languageVersion": "3.4", "name": "app", "packageUri": "lib/", "rootUri": "../"}),
            expect_equal("analyze maps externals under _external", [e["rootUri"] for e in dart_package_entries(_PACKAGES, DART_ANALYZE)], ["../", "../_external/dx_dart_deps+collection", "../_external/repo/sub", "../pkgs/a"]),
            expect_equal("missing language version is omitted", "languageVersion" in dart_package_entries(_PACKAGES, DART_FORMAT)[1], False),
            expect_equal("stub lists sorted deps without itself", dart_pubspec_stub(_PACKAGES, "lib_a"), 'name: lib_a\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\ndependencies:\n  app: any\n  collection: any\n  sub: any\n'),
            expect_equal("stub takes the package language version", dart_pubspec_stub(_PACKAGES, "app").split("\n")[2], '  sdk: "^3.4.0"'),
            expect_equal("stubs sit at in-repo roots", [path for path, _ in dart_pubspec_stubs(_PACKAGES)], ["pubspec.yaml", "pkgs/a/pubspec.yaml"]),
        ],
    )
