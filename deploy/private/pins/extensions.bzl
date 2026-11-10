"""Module extension for deploy/private/pins."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", "http_file")
load("@dx_pins_data//:pins.bzl", "DATA")

_ARCHIVES = DATA["dx_deploy"]["archives"]

def _platforms(prefix):
    return {name[len(prefix):]: pin for name, pin in _ARCHIVES.items() if name.startswith(prefix)}

def _semver_checks_impl(mctx):
    """Declares one pinned cargo-semver-checks archive per supported platform."""
    for key, pin in _platforms("semver_checks_").items():
        http_archive(
            name = "cargo_semver_checks_" + key,
            build_file_content = 'exports_files(["%s"])\n' % pin["exe"],
            sha256 = pin["sha256"],
            urls = pin["urls"],
        )
    return mctx.extension_metadata(reproducible = True)

semver_checks = module_extension(
    implementation = _semver_checks_impl,
    doc = "Creates the pinned cargo-semver-checks binary for each supported platform.",
)

def _japicmp_impl(mctx):
    """Declares the pinned japicmp jar with its dependencies."""
    http_file(
        name = "japicmp_jar",
        downloaded_file_path = "japicmp.jar",
        sha256 = _ARCHIVES["japicmp"]["sha256"],
        urls = _ARCHIVES["japicmp"]["urls"],
    )
    return mctx.extension_metadata(reproducible = True)

japicmp = module_extension(
    implementation = _japicmp_impl,
    doc = "Creates the pinned japicmp jar.",
)

def _apicompat_impl(mctx):
    """Declares the pinned ApiCompat tool package."""
    http_archive(
        name = "apicompat_tool",
        build_file_content = 'filegroup(name = "files", srcs = glob(["tools/net8.0/any/**"]), visibility = ["//visibility:public"])\n',
        sha256 = _ARCHIVES["apicompat"]["sha256"],
        type = "zip",
        urls = _ARCHIVES["apicompat"]["urls"],
    )
    return mctx.extension_metadata(reproducible = True)

apicompat = module_extension(
    implementation = _apicompat_impl,
    doc = "Creates the pinned Microsoft.DotNet.ApiCompat.Tool package.",
)

def _opam_impl(mctx):
    """Declares one pinned opam executable per supported platform."""
    for key, pin in _platforms("opam_").items():
        http_file(
            name = "opam_" + key,
            downloaded_file_path = pin["exe"],
            executable = True,
            sha256 = pin["sha256"],
            urls = pin["urls"],
        )
    return mctx.extension_metadata(reproducible = True)

opam = module_extension(
    implementation = _opam_impl,
    doc = "Creates the pinned opam executable for each supported platform.",
)

def _conan_impl(mctx):
    """Declares one pinned conan distribution per supported platform."""
    for key, pin in _platforms("conan_").items():
        http_archive(
            name = "conan_" + key,
            build_file_content = 'exports_files(["%s"])\nfilegroup(name = "files", srcs = glob(["bin/**"]), visibility = ["//visibility:public"])\n' % pin["exe"],
            sha256 = pin["sha256"],
            urls = pin["urls"],
        )
    return mctx.extension_metadata(reproducible = True)

conan = module_extension(
    implementation = _conan_impl,
    doc = "Creates the pinned conan distribution for each supported platform.",
)
