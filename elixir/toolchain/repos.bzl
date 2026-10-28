"""Hermetic Elixir acquisition and toolchain registration."""

load("//elixir/rules:hex.bzl", "hex_extract")
load("//erlang/toolchain:pins.bzl", "CELLS")
load(
    ":pins.bzl",
    "CREDO_PACKAGES",
    "DIALYZER_PLT_ELIXIR",
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

CREDO_NAME = "dx_credo"

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
{dialyzer_beams}"""

_DIALYZER_BEAMS = """
filegroup(
    name = "dialyzer_beams_{app}",
    srcs = glob(["lib/{app}/ebin/*.beam"]),
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

_CREDO_ROOT = '"../../../../"'

# Credo reads Mix at compile time and its default files from the working
# directory; elixirc runs outside Mix, so each site is pinned to a literal.
CREDO_REWRITES = {
    "bunt": {
        "lib/bunt.ex": [("@version Mix.Project.config()[:version]", '@version "{version}"')],
    },
    "credo": {
        "lib/credo.ex": [("@version Mix.Project.config()[:version]", '@version "{version}"')],
        "lib/credo/check.ex": [
            ("app = Mix.Project.config()[:app]", "app = :credo"),
            ('Code.eval_file(".credo.exs")', 'Code.eval_file(Path.expand("../../.credo.exs", __DIR__))'),
        ],
        "lib/credo/cli/command/gen.check.ex": [("File.read!(@check_template_filename)", "File.read!(Path.expand(" + _CREDO_ROOT + " <> @check_template_filename, __DIR__))")],
        "lib/credo/cli/command/gen.config.ex": [("File.read!(@config_filename)", "File.read!(Path.expand(" + _CREDO_ROOT + " <> @config_filename, __DIR__))")],
        "lib/credo/cli/output/ui.ex": [("if Mix.env() == :test do", "if false do")],
        "lib/credo/execution/task/append_default_config.ex": [("File.read!(@default_config_filename)", "File.read!(Path.expand(" + _CREDO_ROOT + " <> @default_config_filename, __DIR__))")],
        "lib/credo/execution/task/write_debug_report.ex": [('File.read!(".template.debug.html")', "File.read!(Path.expand(" + _CREDO_ROOT + ' <> ".template.debug.html", __DIR__))')],
    },
}

def credo_rewrites(package, path, text, version):
    """Returns one Credo-family source with its compile-time Mix reads pinned; fails unless each marker appears once."""
    for old, new in CREDO_REWRITES.get(package, {}).get(path, []):
        if text.count(old) != 1:
            fail("{} {} no longer has exactly one `{}`".format(package, path, old))
        text = text.replace(old, new.replace("{version}", version))
    return text

_CREDO_BUILD = """load("@rules_dx//elixir/rules:defs.bzl", "ELIXIR_COMPATIBLE_WITH", "elixir_app")

package(default_visibility = ["//visibility:public"])

elixir_app(
    app_name = "{package}",
    app_version = "{version}",
    data = glob([".*", "LICENSE*"], allow_empty = True),
    deps = [{deps}],
    elixirc_opts = [],
    srcs = glob(["lib/**/*.ex"], exclude = ["lib/mix/**"]),
    target_compatible_with = ELIXIR_COMPATIBLE_WITH,
)
"""

_CREDO_DEPS = {
    "bunt": [],
    "credo": ["bunt", "file_system", "jason"],
    "file_system": [],
    "jason": [],
}

def credo_build(package):
    """Returns the BUILD file of one vendored Credo-family package."""
    deps = ", ".join(['"//{}:erlang_app"'.format(dep) for dep in _CREDO_DEPS[package]])
    return _CREDO_BUILD.format(package = package, version = CREDO_PACKAGES[package].version, deps = deps)

def _credo_impl(rctx):
    for package, pin in CREDO_PACKAGES.items():
        hex_extract(rctx, package, pin.version, pin.sha256, output = package)
        for path in CREDO_REWRITES.get(package, {}):
            rctx.file(package + "/" + path, credo_rewrites(package, path, rctx.read(package + "/" + path), pin.version))
        rctx.file(package + "/BUILD.bazel", credo_build(package))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "")
    return rctx.repo_metadata(reproducible = True)

_credo = repository_rule(
    implementation = _credo_impl,
    doc = "Builds the pinned Credo and its dependencies from Hex sources.",
)

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
    rctx.file("BUILD.bazel", _DIST_BUILD.format(version_file = VERSION_FILE, dialyzer_beams = "".join([_DIALYZER_BEAMS.format(app = app) for app in DIALYZER_PLT_ELIXIR])))
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
    _credo(name = CREDO_NAME)
    return mctx.extension_metadata(reproducible = True)

elixir = module_extension(
    implementation = _elixir_impl,
    doc = "Creates rules_elixir, the pinned Elixir tree and the toolchain hub.",
)
