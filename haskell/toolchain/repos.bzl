"""Hermetic GHC acquisition and toolchain registration."""

load("//libs/starlark:deb_closure.bzl", "deb_closure")
load(
    ":pins.bzl",
    "CELLS",
    "GHC_VERSION",
    "RULES_HASKELL_COMMIT",
    "RULES_HASKELL_SHA256",
    "RULES_HASKELL_URL",
    "RUNTIME_MIRROR",
)

HUB_NAME = "dx_haskell_toolchains"

CLOSURE_DIR = "dx_closure"

LIBDIR = "lib"

VERSION_FILE = "DX_GHC_VERSION"

GHC_LIBRARY_DIR = LIBDIR + "/x86_64-linux-ghc-" + GHC_VERSION

GMP_PACKAGE = "ghc-bignum"

_CELL_PREFIX = "dx_ghc_"

# The boot packages the hub exposes as `@dx_haskell_toolchains//:<package>`.
TOOLCHAIN_LIBRARIES = [
    "array",
    "base",
    "binary",
    "bytestring",
    "containers",
    "deepseq",
    "directory",
    "exceptions",
    "filepath",
    "ghc-prim",
    "mtl",
    "parsec",
    "pretty",
    "process",
    "stm",
    "template-haskell",
    "text",
    "time",
    "transformers",
    "unix",
]

DROPPED_PATHS = [
    "BUILD.bazel",
    "CLAUDE.md",
    "MODULE.bazel",
    "WORKSPACE",
    "WORKSPACE.bzlmod",
    "buildifier",
    "constants.bzl",
    "debug",
    "docs",
    "examples",
    "extensions",
    "haskell/ahc.BUILD.tpl",
    "haskell/assets/ghc_9_2_1_mac.patch",
    "haskell/assets/relpath.sh",
    "haskell/asterius",
    "haskell/c2hs",
    "haskell/c2hs.bzl",
    "haskell/cabal",
    "haskell/cabal.bzl",
    "haskell/cabal_wrapper.bzl",
    "haskell/doctest.bzl",
    "haskell/gen_ghc_bindist.py",
    "haskell/ghc.bzl",
    "haskell/ghc_bindist.bzl",
    "haskell/haddock.bzl",
    "haskell/nixpkgs.bzl",
    "haskell/private/cabal_wrapper.py",
    "haskell/private/ghc_bindist_generated.json",
    "haskell/private/ghc_ci.bzl",
    "haskell/private/pkgdb_to_bzl.bzl",
    "haskell/private/stack_snapshot_pin.sh.tpl",
    "haskell/private/workspace_utils.bzl",
    "haskell/protobuf.bzl",
    "haskell/repositories.bzl",
    "logo",
    "netlify.toml",
    "nixpkgs",
    "non_module_dev_deps.bzl",
    "protobuf",
    "registry",
    "rule_info",
    "rules_haskell_nix",
    "rules_haskell_tests",
    "run-bazel",
    "serve-docs.sh",
    "shell.nix",
    "stackage_snapshot.json",
    "stackage_snapshot.yaml",
    "stackage_snapshot_9.10.3.json",
    "stackage_snapshot_9.10.3.yaml",
    "stackage_snapshot_9.2.8.json",
    "stackage_snapshot_9.2.8.yaml",
    "stackage_snapshot_9.4.8.json",
    "stackage_snapshot_9.4.8.yaml",
    "stackage_snapshot_9.6.5.json",
    "stackage_snapshot_9.6.5.yaml",
    "stackage_snapshot_9.8.2.json",
    "stackage_snapshot_9.8.2.yaml",
    "start",
    "tests",
    "tools/BUILD.bazel",
    "tools/README.md",
    "tools/coverage-reports",
    "tools/ghc-paths",
    "tools/os_info.bzl",
    "tools/repositories.bzl",
    "tools/worker",
    "tutorial",
]

GHC_DROPPED_PATHS = [
    "INSTALL",
    "Makefile",
    "README",
    "build.mk",
    "completion",
    "config.guess",
    "config.mk.in",
    "config.sub",
    "configure",
    "default.host.target.in",
    "default.target.in",
    "doc",
    "docs-utils",
    "install-sh",
    LIBDIR + "/html",
    LIBDIR + "/latex",
    "manpage",
    "mk",
    "share",
    "wrappers",
]

# Every program GHC would otherwise take from the build host. The Haskell rules
# pass the C compiler, assembler, linker and object merger of the registered
# cc toolchain on each invocation; anything else fails with this name.
NO_HOST_TOOL = "dx-no-host-tool"

SETTINGS = {
    "C compiler command": NO_HOST_TOOL,
    "C compiler flags": "",
    "C++ compiler command": NO_HOST_TOOL,
    "C++ compiler flags": "",
    "C compiler link flags": "",
    "CPP command": NO_HOST_TOOL,
    "Haskell CPP command": NO_HOST_TOOL,
    "JavaScript CPP command": NO_HOST_TOOL,
    "C-- CPP command": NO_HOST_TOOL,
    "Merge objects command": NO_HOST_TOOL,
    "ar command": NO_HOST_TOOL,
    "ranlib command": NO_HOST_TOOL,
    "otool command": NO_HOST_TOOL,
    "install_name_tool command": NO_HOST_TOOL,
    "windres command": NO_HOST_TOOL,
    "LLVM llc command": NO_HOST_TOOL,
    "LLVM opt command": NO_HOST_TOOL,
    "LLVM llvm-as command": NO_HOST_TOOL,
}

_ROOT_BUILD = """exports_files(["LICENSE"])
"""

_GHC_BUILD = """load("@rules_cc//cc:cc_import.bzl", "cc_import")
load("@rules_haskell//haskell:defs.bzl", "haskell_import", "haskell_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["{version_file}"])

{libraries}

haskell_toolchain(
    name = "toolchain-impl",
    docdir_path = "doc",
    fully_static_link = False,
    libdir = [":lib"],
    libraries = toolchain_libraries,
    locale = "C.UTF-8",
    static_runtime = False,
    tools = [":bin"],
    version = "{version}",
)

filegroup(
    name = "bin",
    srcs = glob(["bin/*"]),
)

filegroup(
    name = "lib",
    srcs = glob(["{libdir}/**", "{closure}/**"]),
)

cc_import(
    name = "gmp",
    shared_library = "{closure}/lib/libgmp.so.10",
)

filegroup(
    name = "licenses",
    srcs = glob(["{closure}/licenses/*"]),
)
"""

_TOOLCHAIN = """toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:toolchain-impl",
    toolchain_type = "@rules_haskell//haskell:toolchain",
)
"""

_HUB_ALIAS = """alias(
    name = "{name}",
    actual = select({{
{arms}
    }}),
)
"""

def cell_repository_name(cell):
    """Returns the repository name one qualified cell is acquired into."""
    return _CELL_PREFIX + cell

def ghc_settings(source):
    """Returns GHC's lib/settings with every host program replaced.

    Args:
      source: the settings file the bindist ships.

    Returns:
      The settings text, which names no host path.
    """
    lines = []
    seen = []
    for line in source.split("\n"):
        stripped = line.lstrip("[,").strip()
        if not stripped.startswith("(\""):
            lines.append(line)
            continue
        key = stripped[2:stripped.index("\"", 2)]
        if key in SETTINGS:
            seen.append(key)
            line = line[:line.index("(")] + "(" + repr(key) + ", " + repr(SETTINGS[key]) + ")"
        lines.append(line)
    missing = [key for key in SETTINGS if key not in seen]
    if missing:
        fail("GHC settings no longer carry " + ", ".join(missing))
    settings = "\n".join(lines)
    if "\"/" in settings:
        fail("GHC settings still name a host path:\n" + settings)
    return settings

def closure_library_dirs(conf, closure):
    """Returns one package registration whose libraries also resolve in the closure.

    Args:
      conf: the package .conf text.
      closure: the closure directory relative to the repository root.

    Returns:
      The conf text with the closure lib directory added to its library dirs.
    """
    entry = "${pkgroot}/../" + closure + "/lib"
    out = []
    found = 0
    for line in conf.split("\n"):
        if line.startswith("library-dirs:") or line.startswith("dynamic-library-dirs:"):
            field, _, value = line.partition(":")
            line = field + ": " + entry + " " + value.strip()
            found += 1
        out.append(line)
    if found != 2:
        fail("package registration has no library-dirs and dynamic-library-dirs pair")
    return "\n".join(out)

def _execute(rctx, arguments, **kwargs):
    result = rctx.execute(arguments, environment = {"LANG": "C.UTF-8"}, **kwargs)
    if result.return_code != 0:
        fail("{} failed ({}):\n{}{}".format(arguments[0], result.return_code, result.stdout, result.stderr))
    return result

def _rules_haskell_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_haskell-" + rctx.attr.commit,
    )
    for path in DROPPED_PATHS:
        if not rctx.path(path).exists:
            fail("rules_haskell no longer ships " + path)
        rctx.delete(path)
    rctx.file("BUILD.bazel", _ROOT_BUILD)
    rctx.file("REPO.bazel", "")
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

_rules_haskell = repository_rule(
    implementation = _rules_haskell_impl,
    doc = "Fetches the pinned rules_haskell sources with the hermetic GHC patch.",
    attrs = {
        "commit": attr.string(mandatory = True, doc = "Pinned rules_haskell commit."),
        "patches": attr.label_list(doc = "Patches applied after extraction."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
    },
)

def _gmp_registration(rctx):
    confs = [entry for entry in rctx.path(LIBDIR + "/package.conf.d").readdir() if entry.basename.startswith(GMP_PACKAGE + "-")]
    if len(confs) != 1:
        fail("GHC ships no single {} registration".format(GMP_PACKAGE))
    conf = confs[0]
    rctx.file(conf, closure_library_dirs(rctx.read(conf), CLOSURE_DIR), executable = False)

def _ghc_repo_impl(rctx):
    cell = CELLS[rctx.attr.cell]
    rctx.download_and_extract(
        url = cell.ghc_url,
        sha256 = cell.ghc_sha256,
        stripPrefix = cell.ghc_strip_prefix,
    )
    for path in GHC_DROPPED_PATHS:
        rctx.delete(path)
    deb_closure(rctx, RUNTIME_MIRROR, cell.runtime, CLOSURE_DIR)
    rctx.symlink(CLOSURE_DIR + "/lib/libgmp.so.10", CLOSURE_DIR + "/lib/libgmp.so")
    for package in cell.runtime:
        for soname in package.libraries:
            rctx.symlink(CLOSURE_DIR + "/lib/" + soname, GHC_LIBRARY_DIR + "/" + soname)
    rctx.file(LIBDIR + "/settings", ghc_settings(rctx.read(LIBDIR + "/settings")), executable = False)
    _gmp_registration(rctx)
    _execute(rctx, ["bin/ghc-pkg", "recache", "--global"])
    version = _execute(rctx, ["bin/ghc", "--numeric-version"]).stdout.strip()
    if version != GHC_VERSION:
        fail("GHC bindist reports {}, not {}".format(version, GHC_VERSION))
    python = rctx.path(rctx.attr._python)
    script = rctx.path(rctx.attr._pkgdb_to_bzl)
    result = json.decode(_execute(rctx, [python, "-E", "-s", "-B", script, rctx.name, LIBDIR]).stdout)
    if "#SYMLINK:" in result["file_content"]:
        fail("GHC package database points outside the bindist")
    rctx.file(VERSION_FILE, GHC_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _GHC_BUILD.format(
        closure = CLOSURE_DIR,
        libdir = LIBDIR,
        libraries = result["file_content"],
        version = GHC_VERSION,
        version_file = VERSION_FILE,
    ))
    return rctx.repo_metadata(reproducible = True)

_ghc_repo = repository_rule(
    implementation = _ghc_repo_impl,
    doc = "Acquires one relocatable GHC bindist plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
        "_pkgdb_to_bzl": attr.label(default = Label("@rules_haskell//haskell:private/pkgdb_to_bzl.py")),
        "_python": attr.label(default = Label("@python_3_12_x86_64_unknown_linux_gnu//:bin/python3")),
    },
)

def _hub_impl(rctx):
    content = []
    arms = []
    for cell in sorted(CELLS.keys()):
        content.append(_TOOLCHAIN.format(
            cell = cell,
            constraints = repr(CELLS[cell].exec_compatible_with),
            repo = cell_repository_name(cell),
        ))
        content.append("config_setting(\n    name = \"{}_platform\",\n    constraint_values = {},\n)\n".format(
            cell,
            repr(CELLS[cell].exec_compatible_with),
        ))
        arms.append("        \":{}_platform\": \"@{}//:{{name}}\",".format(cell, cell_repository_name(cell)))
    for name in ["gmp", "licenses"]:
        content.append(_HUB_ALIAS.format(name = name, arms = "\n".join(arms).replace("{name}", name)))
    rctx.file("REPO.bazel", "")
    for name in TOOLCHAIN_LIBRARIES:
        content.append("haskell_toolchain_library(name = \"{}\")\n".format(name))
    rctx.file("BUILD.bazel", "load(\"@rules_haskell//haskell:defs.bzl\", \"haskell_toolchain_library\")\n\npackage(default_visibility = [\"//visibility:public\"])\n\n" + "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares one Haskell toolchain per qualified host cell and the runtime closure aliases.",
)

def _haskell_impl(mctx):
    _rules_haskell(
        name = "rules_haskell",
        commit = RULES_HASKELL_COMMIT,
        patches = [Label("//patches/rules_haskell:hermetic_haskell.patch")],
        sha256 = RULES_HASKELL_SHA256,
        url = RULES_HASKELL_URL,
    )
    for cell in sorted(CELLS.keys()):
        _ghc_repo(name = cell_repository_name(cell), cell = cell)
    _hub(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

haskell = module_extension(
    implementation = _haskell_impl,
    doc = "Creates rules_haskell, the pinned GHC bindists and the toolchain hub.",
)
