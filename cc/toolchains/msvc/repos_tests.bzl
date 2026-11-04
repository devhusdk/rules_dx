"""MSVC layout assembly coverage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":layout.bzl", "msvc_layout")
load(":repos.bzl", "HUB_NAME", "LICENSE", "UNACCEPTED", "hub_build", "layout_links", "missing_error")
load(":unavailable.bzl", "msvc_unavailable")

def _checks():
    links = layout_links("14.44.35207", "10.0.26100.0")
    layout = msvc_layout("14.44.35207", "10.0.26100.0")
    layout_paths = [layout.cl, layout.link, layout.lib, layout.ml64] + layout.include_dirs + layout.lib_dirs
    sdk = "Windows Kits/10/"
    return [
        expect_equal("hub name", HUB_NAME, "dx_msvc"),
        expect_equal("link destinations", [l.dst for l in links], [
            "VC/Tools/MSVC/14.44.35207",
            sdk + "Include/10.0.26100.0",
            sdk + "Lib/10.0.26100.0/ucrt/x64",
            sdk + "Lib/10.0.26100.0/um/x64",
        ]),
        expect_equal("link sources", [(l.repo, l.src) for l in links], [
            ("msvc_runtime", "sysroot/Contents/VC/Tools/MSVC/14.44.35207"),
            ("windows_sdk", "sysroot/base/c/Include/10.0.26100.0"),
            ("windows_sdk", "sysroot/x64/c/ucrt/x64"),
            ("windows_sdk", "sysroot/x64/c/um/x64"),
        ]),
        expect_true("every link feeds the layout", all([any([p == l.dst or p.startswith(l.dst + "/") for p in layout_paths]) for l in links])),
        expect_equal("missing text", missing_error(["a/cl.exe", "b/link.exe"]), "dx_msvc: incomplete MSVC payload; missing a/cl.exe, b/link.exe"),
        expect_equal(
            "unaccepted text",
            UNACCEPTED,
            "dx_msvc: the Visual Studio Build Tools license (https://visualstudio.microsoft.com/license-terms/) is not accepted; add msvc.license(accept = [\"visual-studio-build-tools\"]) to the root MODULE.bazel",
        ),
        expect_equal("license name", LICENSE, "visual-studio-build-tools"),
        expect_contains("accepted hub builds the toolchain", hub_build(True, "14.44.35207", "10.0.26100.0"), 'msvc_version = "14.44.35207"'),
        expect_contains("accepted hub names the sdk", hub_build(True, "14.44.35207", "10.0.26100.0"), 'sdk_version = "10.0.26100.0"'),
        expect_true("accepted hub has no error", "error =" not in hub_build(True, "14.44.35207", "10.0.26100.0")),
        expect_contains("unaccepted hub carries the error", hub_build(False, "", ""), "error = " + json.encode(UNACCEPTED)),
        expect_true("unaccepted hub builds no toolchain", "msvc_cc_toolchain(" not in hub_build(False, "", "")),
    ]

def msvc_repos_tests(name):
    """Declares the MSVC layout assembly test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )

def _unaccepted_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, UNACCEPTED)
    return analysistest.end(env)

_unaccepted_test = analysistest.make(_unaccepted_impl, expect_failure = True)

def msvc_unavailable_tests(name):
    """Declares the test that an unaccepted MSVC layout fails when used."""
    msvc_unavailable(
        name = name + "_target",
        error = UNACCEPTED,
        tags = ["manual"],
    )
    _unaccepted_test(
        name = name,
        target_under_test = ":" + name + "_target",
    )
