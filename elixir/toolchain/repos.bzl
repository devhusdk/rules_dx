"""Hermetic Elixir acquisition and toolchain registration."""

load("//erlang/toolchain:pins.bzl", "CELLS")
load(
    ":pins.bzl",
    "ELIXIR_APPLICATIONS",
    "ELIXIR_CELLS",
    "ELIXIR_SHA256",
    "ELIXIR_URL",
    "ELIXIR_VERSION",
    "RULES_ELIXIR_COMMIT",
    "RULES_ELIXIR_SHA256",
    "RULES_ELIXIR_URL",
)

HUB_NAME = "dx_elixir_toolchains"

DIST_NAME = "dx_elixir"

VERSION_FILE = "DX_ELIXIR_VERSION"

DROPPED_PATHS = ["man", "Makefile", "bin/elixir.bat", "bin/elixirc.bat", "bin/iex.bat", "bin/mix.bat", "bin/mix.ps1"]

ERL_OPTIONS_MARKER = "$ELIXIR_ERL_OPTIONS $ERL"

UNICODE_ERL_OPTIONS = "+fnu -kernel standard_io_encoding unicode "

def elixir_script(source):
    """Returns the elixir launcher with Unicode file names and stdio whatever the locale."""
    if source.count(ERL_OPTIONS_MARKER) != 1:
        fail("bin/elixir no longer passes ELIXIR_ERL_OPTIONS once")
    return source.replace(ERL_OPTIONS_MARKER, UNICODE_ERL_OPTIONS + ERL_OPTIONS_MARKER)

_DIST_BUILD = """load("@rules_dx//elixir/toolchain:elixir.bzl", "dx_elixir")
load("@rules_elixir//:elixir_toolchain.bzl", "elixir_toolchain")

package(default_visibility = ["//visibility:public"])

dx_elixir(
    name = "elixir_tree",
    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"]),
    root = "{version_file}",
)

elixir_toolchain(
    name = "elixir",
    elixir = ":elixir_tree",
)
"""

_TOOLCHAIN = """toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:elixir",
    toolchain_type = "@rules_elixir//:toolchain_type",
)
"""

def _rules_elixir_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_elixir-" + rctx.attr.commit,
    )
    for path in ["MODULE.bazel", "examples", "test", ".bazelversion", ".bazelrc", ".bazelignore"]:
        rctx.delete(path)
    rctx.file("REPO.bazel", "")
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

_rules_elixir = repository_rule(
    implementation = _rules_elixir_impl,
    doc = "Fetches the pinned rules_elixir sources with the hermetic Elixir patch.",
    attrs = {
        "commit": attr.string(mandatory = True, doc = "Pinned rules_elixir commit."),
        "patches": attr.label_list(doc = "Patches applied after extraction."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
    },
)

def _dist_impl(rctx):
    rctx.download_and_extract(url = ELIXIR_URL, sha256 = ELIXIR_SHA256, type = "zip")
    if rctx.read("VERSION").strip() != ELIXIR_VERSION:
        fail("Elixir archive is not release " + ELIXIR_VERSION)
    for app in ELIXIR_APPLICATIONS:
        if not rctx.path("lib/" + app + "/ebin/" + app + ".app").exists:
            fail("Elixir archive has no " + app + " application")
    for path in DROPPED_PATHS:
        rctx.delete(path)
    rctx.file("bin/elixir", elixir_script(rctx.read("bin/elixir")), executable = True)
    rctx.file(VERSION_FILE, ELIXIR_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _DIST_BUILD.format(version_file = VERSION_FILE))
    return rctx.repo_metadata(reproducible = True)

_dist = repository_rule(
    implementation = _dist_impl,
    doc = "Acquires the pinned precompiled Elixir distribution.",
)

def hub_build(cells):
    """Returns the hub BUILD file that registers Elixir on the given OTP cells."""
    return "\n".join([_TOOLCHAIN.format(
        cell = cell,
        constraints = repr(CELLS[cell].exec_compatible_with),
        repo = DIST_NAME,
    ) for cell in cells])

def _hub_impl(rctx):
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", hub_build(ELIXIR_CELLS))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares one Elixir toolchain per qualified OTP cell.",
)

def _elixir_impl(mctx):
    _rules_elixir(
        name = "rules_elixir",
        commit = RULES_ELIXIR_COMMIT,
        # The combined diff of the upstream rules_elixir PRs drafted in #3744.
        patches = [Label("//patches/rules_elixir:hermetic_elixir.patch")],
        sha256 = RULES_ELIXIR_SHA256,
        url = RULES_ELIXIR_URL,
    )
    _dist(name = DIST_NAME)
    _hub(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

elixir = module_extension(
    implementation = _elixir_impl,
    doc = "Creates rules_elixir, the pinned Elixir tree and the toolchain hub.",
)
