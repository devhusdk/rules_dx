"""MSVC layout assembly coverage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cc/toolchains:unavailable.bzl", "cc_toolchain_unavailable")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":layout.bzl", "SUPPORTED_CELLS", "msvc_layout")
load(":repos.bzl", "HUB_NAME", "LICENSE", "NO_REDIST", "UNACCEPTED", "VCVARS_STUBS", "has_child_exact", "hub_build", "host_dir_name", "layout_links", "missing_error")
load(":unavailable.bzl", "UNSUPPORTED")

def _checks():
    links = layout_links("14.44.35207", "10.0.26100.0")
    layouts = [msvc_layout("14.44.35207", "10.0.26100.0", host, target) for host, target in SUPPORTED_CELLS]
    layout_paths = [p for lay in layouts for p in [lay.cl, lay.link, lay.lib, lay.ml] + lay.include_dirs + lay.lib_dirs + lay.bin_dirs if p]
    accepted_hub = hub_build(True, "14.44.35207", "10.0.26100.0", {"cc_toolchain_x64_arm64": missing_error(["a/cl.exe"])})
    redist_hub = hub_build(True, "14.44.35207", "10.0.26100.0", {}, "14.50.35710")
    sdk = "Windows Kits/10/"
    return [
        expect_equal("hub name", HUB_NAME, "dx_msvc"),
        expect_equal("vcvars stubs", VCVARS_STUBS, ["VC/Auxiliary/Build/vcvarsall.bat", "VC/Auxiliary/Build/vcvars64.bat"]),
        expect_equal("link destinations", [k.dst for k in links], [
            "VC/Tools/MSVC/14.44.35207/include",
            "VC/Tools/MSVC/14.44.35207/lib",
            "VC/Tools/MSVC/14.44.35207/bin/Hostarm64",
            "VC/Tools/MSVC/14.44.35207/bin/Hostx64",
            sdk + "Include/10.0.26100.0",
            sdk + "bin/10.0.26100.0/x64",
            sdk + "Lib/10.0.26100.0/ucrt/x64",
            sdk + "Lib/10.0.26100.0/um/x64",
            sdk + "bin/10.0.26100.0/arm64",
            sdk + "Lib/10.0.26100.0/ucrt/arm64",
            sdk + "Lib/10.0.26100.0/um/arm64",
        ]),
        expect_equal("link sources", [(k.repo, k.src) for k in links], [
            ("msvc_runtime", "sysroot/Contents/VC/Tools/MSVC/14.44.35207/include"),
            ("msvc_runtime", "sysroot/Contents/VC/Tools/MSVC/14.44.35207/lib"),
            ("msvc_runtime", "sysroot/Contents/VC/Tools/MSVC/14.44.35207/bin/Hostarm64"),
            ("msvc_runtime", "sysroot/Contents/VC/Tools/MSVC/14.44.35207/bin/Hostx64"),
            ("windows_sdk", "sysroot/base/c/Include/10.0.26100.0"),
            ("windows_sdk", "sysroot/base/c/bin/10.0.26100.0/x64"),
            ("windows_sdk", "sysroot/x64/c/ucrt/x64"),
            ("windows_sdk", "sysroot/x64/c/um/x64"),
            ("windows_sdk", "sysroot/base/c/bin/10.0.26100.0/arm64"),
            ("windows_sdk", "sysroot/arm64/c/ucrt/arm64"),
            ("windows_sdk", "sysroot/arm64/c/um/arm64"),
        ]),
        expect_true("every link feeds the layout", all([any([p == k.dst or p.startswith(k.dst + "/") for p in layout_paths]) for k in links])),
        expect_true("exact case mismatch is missing", not has_child_exact(["HostX64"], "Hostx64")),
        expect_true("exact case match is found", has_child_exact(["Hostx64"], "Hostx64")),
        expect_equal("host dir spellings", [host_dir_name(n) for n in ["HostX64", "Hostx64", "hostarm64", "Hostx86", "x64"]], ["Hostx64", "Hostx64", "Hostarm64", "Hostx86", "x64"]),
        expect_equal("missing text", missing_error(["a/cl.exe", "b/link.exe"]), "dx_msvc: incomplete MSVC payload; missing a/cl.exe, b/link.exe"),
        expect_equal(
            "unaccepted text",
            UNACCEPTED,
            "dx_msvc: the Visual Studio Build Tools license (https://visualstudio.microsoft.com/license-terms/) is not accepted; add msvc.license(accept = [\"visual-studio-build-tools\"]) to the root MODULE.bazel",
        ),
        expect_equal("license name", LICENSE, "visual-studio-build-tools"),
        expect_contains("accepted hub builds the toolchain", accepted_hub, 'msvc_version = "14.44.35207"'),
        expect_contains("accepted hub names the sdk", accepted_hub, 'sdk_version = "10.0.26100.0"'),
        expect_contains("x64 cell", accepted_hub, 'name = "cc_toolchain_x64_x64",\n    msvc_version = "14.44.35207",\n    sdk_version = "10.0.26100.0",\n    exec_cpu = "x64",\n    target_cpu = "x64",'),
        expect_contains("native arm64 cell", accepted_hub, 'name = "cc_toolchain_arm64_arm64",\n    msvc_version = "14.44.35207",\n    sdk_version = "10.0.26100.0",\n    exec_cpu = "arm64",\n    target_cpu = "arm64",'),
        expect_contains("missing tools make only their cell unavailable", accepted_hub, 'cc_toolchain_unavailable(\n    name = "cc_toolchain_x64_arm64",\n    error = ' + json.encode(missing_error(["a/cl.exe"]))),
        expect_equal("one toolchain per remaining cell", accepted_hub.count("msvc_cc_toolchain(\n"), 2),
        expect_equal("unaccepted hub makes every cell and the runtime unavailable", hub_build(False, "", "").count("error = " + json.encode(UNACCEPTED)), 4),
        expect_contains("accepted hub globs the x64 runtime", accepted_hub, 'name = "vcruntime_x64",\n    srcs = glob(["redist/x64/*/vcruntime140.dll","redist/x64/*/vcruntime140_1.dll","redist/x64/*/msvcp140.dll"])'),
        expect_contains("unaccepted hub ships no runtime", hub_build(False, "", ""), 'name = "vcruntime_arm64",\n    srcs = glob([])'),
        expect_true("unaccepted hub builds no toolchain", "msvc_cc_toolchain(" not in hub_build(False, "", "")),
        expect_contains("accepted hub names the runtime version", redist_hub, 'version = "14.50.35710"'),
        expect_contains("accepted hub declares the runtime", redist_hub, 'name = "msvc_vcruntime"'),
        expect_contains("no redist folder cannot ship", hub_build(True, "14.44.35207", "10.0.26100.0"), "error = " + json.encode(NO_REDIST)),
        expect_contains("unaccepted hub fails the runtime", hub_build(False, "", ""), 'name = "msvc_vcruntime",\n    error = ' + json.encode(UNACCEPTED)),
        expect_true("unaccepted hub declares no runtime", "native_runtime(" not in hub_build(False, "", "")),
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
    cc_toolchain_unavailable(
        name = name + "_target",
        error = UNACCEPTED,
        tags = ["manual"],
    )
    _unaccepted_test(
        name = name,
        target_under_test = ":" + name + "_target",
    )

def _unsupported_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, UNSUPPORTED)
    return analysistest.end(env)

_unsupported_test = analysistest.make(_unsupported_impl, expect_failure = True)

def msvc_unsupported_tests(name):
    """Declares the test that the unsupported-platform guard fails when used."""
    cc_toolchain_unavailable(
        name = name + "_target",
        error = UNSUPPORTED,
        tags = ["manual"],
    )
    _unsupported_test(
        name = name,
        target_under_test = ":" + name + "_target",
    )
