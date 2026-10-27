"""D toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":pins.bzl", "LDC_CELLS", "LDC_RELEASES", "LDC_VERSION", "RULES_D_SHA256", "RULES_D_URL", "RULES_D_VERSION")
load(":repos.bzl", "DROPPED_PATHS", "LDC_BUILD", "REQUIRED_PATHS", "WRAPPER", "ldc_runtime")

def _checks():
    linux = LDC_CELLS["linux_amd64"]
    return [
        expect_equal("rules_d version", RULES_D_VERSION, "0.12.1"),
        expect_equal("rules_d digest", len(RULES_D_SHA256), 64),
        expect_contains("rules_d url names the version", RULES_D_URL, "v" + RULES_D_VERSION + ".tar.gz"),
        expect_equal("ldc version", LDC_VERSION, "1.43.0"),
        expect_equal("release url", LDC_RELEASES, "https://github.com/ldc-developers/ldc/releases/download/v1.43.0/"),
        expect_equal("pinned cells", sorted(LDC_CELLS.keys()), ["linux_amd64"]),
        expect_equal("linux archive", linux.archive, "ldc2-1.43.0-linux-x86_64.tar.xz"),
        expect_equal("linux digest", len(linux.sha256), 64),
        expect_contains("linux prefix names the version", linux.strip_prefix, LDC_VERSION),
        expect_equal("linux runtime closure", sorted([package.package for package in ldc_runtime("linux_amd64")]), ["gcc-12-base", "libgcc-s1", "zlib1g"]),
        expect_true("the upstream toolchain extension is dropped", "d/extensions.bzl" in DROPPED_PATHS),
        expect_true("the upstream compiler download is dropped", "d/repositories.bzl" in DROPPED_PATHS),
        expect_true("the upstream module file is dropped", "MODULE.bazel" in DROPPED_PATHS),
        expect_true("acquisition requires the real compiler", "bin/ldc2" in REQUIRED_PATHS),
        expect_contains("ldmd2 sees the real compiler", LDC_BUILD, '"bin/ldc2"'),
        expect_contains("ldmd2 sees the compiler config", LDC_BUILD, '"etc/ldc2.conf/**"'),
        expect_contains("ldmd2 sees the runtime closure", LDC_BUILD, '"{closure}/lib/**"'),
        expect_contains("the wrapper loads the closure", WRAPPER, "/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"),
        expect_contains("the wrapper runs ldmd2", WRAPPER, 'exec "$bin/ldmd2" "$@"'),
    ]

def d_metadata_tests(name):
    """Declares the D toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
