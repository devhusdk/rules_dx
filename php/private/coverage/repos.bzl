"""The Windows coverage PHP: a dynamic PHP with PCOV and the MSVC runtime it links."""

VC_REDIST_VERSION = "14.50.35710"

COVERAGE_PREFIX = "dx_php_coverage_"

COVERAGE_RUNTIME_FILES = ["php.exe", "php8.dll", "ext/php_mbstring.dll", "php_pcov.dll", "vcruntime140.dll"]

COVERAGE_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files({files})

filegroup(
    name = "files",
    srcs = {files},
)
""".format(files = repr(COVERAGE_RUNTIME_FILES))

def _coverage_runtime_impl(rctx):
    for pin in [rctx.attr.php, rctx.attr.pcov]:
        sha256, urls = pin[0], pin[1:]
        if not sha256:
            fail(rctx.name + ": set sha256; an archive without a digest is not pinned")
        rctx.download_and_extract(url = urls, sha256 = sha256, type = "zip")
    redist = rctx.path(rctx.attr._msvc_runtime.same_package_label("BUILD.bazel")).dirname.get_child("sysroot/Contents/VC/Redist/MSVC/{}/x64".format(VC_REDIST_VERSION))
    if redist.exists:
        for directory in redist.readdir():
            for dll in directory.readdir():
                if dll.basename.lower() == "vcruntime140.dll":
                    rctx.symlink(dll, "vcruntime140.dll")
    for path in COVERAGE_RUNTIME_FILES:
        if not rctx.path(path).exists:
            fail(rctx.name + ": the coverage runtime has no " + path)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", COVERAGE_BUILD)
    return rctx.repo_metadata(reproducible = True)

php_coverage_runtime = repository_rule(
    implementation = _coverage_runtime_impl,
    doc = "Fetches a dynamic PHP with PCOV and places the MSVC runtime it links beside php.exe.",
    attrs = {
        "pcov": attr.string_list(mandatory = True, doc = "The PCOV zip: its sha256 and then its URLs."),
        "php": attr.string_list(mandatory = True, doc = "The PHP zip: its sha256 and then its URLs."),
        "_msvc_runtime": attr.label(default = "@dx_msvc_runtime//:BUILD.bazel", doc = "The hermetic MSVC redistributable."),
    },
)

HEADERS_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

cc_library(
    name = "headers",
    hdrs = glob(["include/**/*.h"]),
    includes = {includes},
)
"""
