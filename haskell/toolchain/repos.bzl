"""Hermetic GHC acquisition and toolchain registration."""

load("//core:deb_closure.bzl", "deb_closure")
load("//core:toolchain.bzl", "toolchain_target")
load(":pins.bzl", "CELLS", "GHC_VERSION", "RUNTIME_MIRROR", "qualified_cells")

HUB_NAME = "dx_haskell_toolchains"

TOOLCHAINS_PACKAGE = "toolchains"

CC_PACKAGE = "cc"

CLOSURE_DIR = "dx_closure"

LIBDIR = "lib"

VERSION_FILE = "DX_GHC_VERSION"

def ghc_library_dir(cell):
    """Returns the bindist library directory of one cell."""
    return LIBDIR + "/" + CELLS[cell].ghc_libdir_triple + "-ghc-" + GHC_VERSION

def cell_matches_host(cell, os_name, arch):
    """Returns whether the cell's binaries run on the host.

    Args:
        cell: Cell key.
        os_name: Host rctx.os.name.
        arch: Host rctx.os.arch.

    Returns:
        True if the host can run the cell's bindist.
    """
    host = CELLS[cell].host
    return any([os_name.lower().startswith(os) for os in host.os]) and arch in host.arch

_UNAVAILABLE_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "toolchain-impl",
    target_compatible_with = ["@platforms//:incompatible"],
)

filegroup(
    name = "gmp",
    target_compatible_with = ["@platforms//:incompatible"],
)

filegroup(
    name = "licenses",
    target_compatible_with = ["@platforms//:incompatible"],
)

filegroup(
    name = "cc-compiler-mingw64",
    target_compatible_with = ["@platforms//:incompatible"],
)

filegroup(
    name = "cabal",
    target_compatible_with = ["@platforms//:incompatible"],
)

filegroup(
    name = "ghc_exe",
    target_compatible_with = ["@platforms//:incompatible"],
)

filegroup(
    name = "ghc_files",
    target_compatible_with = ["@platforms//:incompatible"],
)
"""

GMP_PACKAGE = "ghc-bignum"

_CELL_PREFIX = "dx_ghc_"

COMMON_LIBRARIES = [
    "array",
    "base",
    "binary",
    "bytestring",
    "containers",
    "deepseq",
    "directory",
    "exceptions",
    "filepath",
    "ghc",
    "ghc-bignum",
    "ghc-internal",
    "ghc-prim",
    "hpc",
    "mtl",
    "os-string",
    "parsec",
    "pretty",
    "process",
    "stm",
    "template-haskell",
    "text",
    "time",
    "transformers",
]

def os_libraries():
    """Returns the sorted libraries that only some cells ship."""
    return sorted({library: True for cell in CELLS.values() for library in cell.os_libraries}.keys())

def toolchain_libraries(cell):
    """Returns the sorted libraries one cell's bindist ships."""
    return sorted(COMMON_LIBRARIES + list(CELLS[cell].os_libraries))

WINDOWS_DROPPED_IMPORT_LIBS = [
    "libstdc++.dll.a",
    "libpthread.dll.a",
    "libwinpthread.dll.a",
    "libz.dll.a",
    "libc++.dll.a",
]

def dropped_import_libs(paths):
    """Returns the paths whose file name is a dropped Windows import library."""
    return [path for path in paths if path.rsplit("/", 1)[-1] in WINDOWS_DROPPED_IMPORT_LIBS]

def host_path(settings):
    """Returns whether settings text quotes an absolute host path."""
    if "\"/" in settings:
        return True
    for drive in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".elems():
        if "\"" + drive + ":" in settings:
            return True
    return False

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
    "haskell/doctest.bzl",
    "haskell/gen_ghc_bindist.py",
    "haskell/ghc.bzl",
    "haskell/ghc_bindist.bzl",
    "haskell/nixpkgs.bzl",
    "haskell/private/ghc_bindist_generated.json",
    "haskell/private/ghc_ci.bzl",
    "haskell/private/pkgdb_to_bzl.bzl",
    "haskell/private/stack_snapshot_pin.sh.tpl",
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

NO_HOST_TOOL = "dx-no-host-tool"

BUNDLED_TOOLS = ["Merge objects command", "windres command"]

MINGW_ROOT = "$topdir/../mingw/"

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

ROOT_BUILD = """exports_files(["LICENSE"])
"""

_GHC_BUILD = """load("@rules_cc//cc:cc_import.bzl", "cc_import")
load("@rules_cc//cc:cc_library.bzl", "cc_library")
{cc_loads}load("@rules_haskell//haskell:defs.bzl", "haskell_import", "haskell_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["{version_file}"])

{libraries}

haskell_toolchain(
    name = "toolchain-impl",
    docdir_path = "doc",
    fully_static_link = False,
    libdir = [":lib"],
    libraries = toolchain_libraries,
    locale = "{locale}",
    static_runtime = {static_runtime},
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

filegroup(
    name = "cabal",
    srcs = ["cabal/cabal{exe_suffix}"],
)

filegroup(
    name = "ghc_exe",
    srcs = ["bin/ghc{exe_suffix}"],
)

filegroup(
    name = "ghc_files",
    srcs = [":bin", ":lib"],
)

{gmp}

filegroup(
    name = "licenses",
    srcs = glob(["{closure}/licenses/*"]),
)
{cc}"""

# The bundled mingw clang as a C toolchain. Only the hub's gated toolchain()
# entries select it. Loads sit at the top of the generated file.
_MINGW_CC_LOADS = """load("@rules_cc//cc/toolchains:cc_toolchain.bzl", "cc_toolchain")
load("@rules_haskell//haskell:cc_toolchain_config.bzl", "cc_toolchain_config")
"""

_MINGW_CC_BUILD = """
filegroup(
    name = "mingw",
    srcs = glob(["mingw/**"]),
)

cc_toolchain(
    name = "cc-compiler-mingw64",
    all_files = ":mingw",
    ar_files = ":mingw",
    as_files = ":mingw",
    compiler_files = ":mingw",
    dwp_files = ":mingw",
    linker_files = ":mingw",
    objcopy_files = ":mingw",
    strip_files = ":mingw",
    supports_param_files = 0,
    toolchain_config = ":ghc_windows_mingw64_config",
    toolchain_identifier = "ghc_windows_mingw64",
)

cc_toolchain_config(name = "ghc_windows_mingw64_config", is_clang = True)
"""

_GMP_IMPORT = """cc_import(
    name = "gmp",
    shared_library = "{closure}/lib/libgmp.so.10",
)
"""

_GMP_EMPTY = """cc_library(
    name = "gmp",
)
"""

_HUB_ALIAS = """alias(
    name = "{name}",
    actual = select({{
{arms}
    }}),
)
"""

_OS_LIBRARY = """haskell_toolchain_library(
    name = "{name}",
    target_compatible_with = select({{
{arms}        "//conditions:default": ["{incompatible}"],
    }}),
)
"""

def cell_repository_name(cell):
    """Returns the repository name one qualified cell is acquired into."""
    return _CELL_PREFIX + cell

def ghc_settings(source):
    """Returns the bindist's lib/settings text with every host program replaced."""
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
            if key in BUNDLED_TOOLS and (MINGW_ROOT in source or "\"$tooldir/" in stripped):
                lines.append(line)
                continue
            line = line[:line.index("(")] + "(" + repr(key) + ", " + repr(SETTINGS[key]) + ")"
        lines.append(line)
    missing = [key for key in SETTINGS if key not in seen]
    if missing:
        fail("GHC settings no longer carry " + ", ".join(missing))
    settings = "\n".join(lines)
    if host_path(settings):
        fail("GHC settings still name a host path:\n" + settings)
    return settings

DARWIN_SDK_PREFIX = "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/"

def drop_host_sdk_dirs(conf):
    """Returns the package .conf text without the lines that name the host macOS SDK."""
    lines = [line for line in conf.split("\n") if not line.strip().startswith(DARWIN_SDK_PREFIX)]
    return "\n".join(lines)

def closure_library_dirs(conf, closure):
    """Returns the package .conf text with the closure lib directory added to its library dirs."""
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

def _gmp_registration(rctx):
    confs = [entry for entry in rctx.path(LIBDIR + "/package.conf.d").readdir() if entry.basename.startswith(GMP_PACKAGE + "-")]
    if len(confs) != 1:
        fail("GHC ships no single {} registration".format(GMP_PACKAGE))
    conf = confs[0]
    rctx.file(conf, closure_library_dirs(rctx.read(conf), CLOSURE_DIR), executable = False)

def _drop_import_libs(rctx):
    root = rctx.path("mingw")
    if not root.exists:
        fail("Windows bindist ships no mingw directory")
    pending = [root]
    found = []
    for _ in range(100000):
        if not pending:
            break
        directory = pending.pop()
        for entry in directory.readdir():
            if entry.is_dir:
                pending.append(entry)
            else:
                found.append(str(entry))
    if pending:
        fail("mingw directory tree is deeper than expected")
    for path in dropped_import_libs(found):
        rctx.delete(path)

def _ghc_repo_impl(rctx):
    cell = CELLS[rctx.attr.cell]
    if not cell_matches_host(rctx.attr.cell, rctx.os.name, rctx.os.arch):
        # The bindist cannot run here, so fetch nothing and run nothing. Targets
        # stay incompatible, and toolchain resolution picks the host's own cell.
        rctx.file("REPO.bazel", "")
        rctx.file("BUILD.bazel", _UNAVAILABLE_BUILD)
        return rctx.repo_metadata(reproducible = True)
    rctx.download_and_extract(
        url = cell.ghc.urls,
        sha256 = cell.ghc.sha256,
        stripPrefix = cell.ghc.strip_prefix,
    )
    rctx.download_and_extract(
        url = cell.cabal.urls,
        sha256 = cell.cabal.sha256,
        output = "cabal",
    )
    for path in GHC_DROPPED_PATHS:
        rctx.delete(path)
    if cell.runtime:
        deb_closure(rctx, RUNTIME_MIRROR, cell.runtime, CLOSURE_DIR)
        rctx.symlink(CLOSURE_DIR + "/lib/libgmp.so.10", CLOSURE_DIR + "/lib/libgmp.so")
        for package in cell.runtime:
            for soname in package.libraries:
                rctx.symlink(CLOSURE_DIR + "/lib/" + soname, ghc_library_dir(rctx.attr.cell) + "/" + soname)
    rctx.file(LIBDIR + "/settings", ghc_settings(rctx.read(LIBDIR + "/settings")), executable = False)
    if cell.runtime:
        _gmp_registration(rctx)
    gmp_license = getattr(cell, "gmp_license", None)
    if gmp_license:
        rctx.download(
            url = gmp_license.url,
            output = CLOSURE_DIR + "/licenses/" + gmp_license.name,
            sha256 = gmp_license.sha256,
        )
    if cell.exe_suffix == ".exe":
        _drop_import_libs(rctx)
    for entry in rctx.path(LIBDIR + "/package.conf.d").readdir():
        if entry.basename.endswith(".conf"):
            conf = rctx.read(entry)
            stripped = drop_host_sdk_dirs(conf)
            if stripped != conf:
                rctx.file(entry, stripped, executable = False)
    _execute(rctx, ["bin/ghc-pkg" + cell.exe_suffix, "recache", "--global"])
    version = _execute(rctx, ["bin/ghc" + cell.exe_suffix, "--numeric-version"]).stdout.strip()
    if version != GHC_VERSION:
        fail("GHC bindist reports {}, not {}".format(version, GHC_VERSION))
    python = rctx.path(rctx.attr.python)
    script = rctx.path(rctx.attr._pkgdb_to_bzl)
    result = json.decode(_execute(rctx, [python, "-E", "-s", "-B", script, rctx.name, LIBDIR]).stdout)
    if "#SYMLINK:" in result["file_content"]:
        fail("GHC package database points outside the bindist:\n" + "\n".join([line for line in result["file_content"].split("\n") if "#SYMLINK:" in line]))
    mingw_cc = getattr(cell, "mingw_cc", False)
    if mingw_cc and not rctx.path("mingw/bin/clang.exe").exists:
        fail("Windows bindist ships no mingw/bin/clang.exe")
    rctx.file(VERSION_FILE, GHC_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _GHC_BUILD.format(
        cc = _MINGW_CC_BUILD if mingw_cc else "",
        cc_loads = _MINGW_CC_LOADS if mingw_cc else "",
        closure = CLOSURE_DIR,
        exe_suffix = cell.exe_suffix,
        gmp = (_GMP_IMPORT if cell.runtime else _GMP_EMPTY).format(closure = CLOSURE_DIR),
        libdir = LIBDIR,
        libraries = result["file_content"],
        locale = cell.locale,
        static_runtime = cell.exe_suffix == ".exe",
        version = GHC_VERSION,
        version_file = VERSION_FILE,
    ))
    return rctx.repo_metadata(reproducible = True)

ghc_repo = repository_rule(
    implementation = _ghc_repo_impl,
    doc = "Acquires one relocatable GHC bindist plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
        "_pkgdb_to_bzl": attr.label(default = Label("@rules_haskell//haskell:private/pkgdb_to_bzl.py")),
        "python": attr.label(mandatory = True, doc = "Host interpreter for the package database conversion."),
    },
)

def hub_files(cells = CELLS):
    """Returns the hub files: the toolchains, cc toolchains and the root runtime closure aliases.

    Args:
        cells: The cell table, so tests can unblock a cell.

    Returns:
        A dict from hub file path to its content.
    """
    toolchains = []
    content = []
    arms = []
    for cell in qualified_cells(cells):
        toolchains.append(toolchain_target(
            name = cell,
            exec_compatible_with = cells[cell].exec_compatible_with,
            target_compatible_with = cells[cell].exec_compatible_with,
            toolchain = "@{}//:toolchain-impl".format(cell_repository_name(cell)),
            toolchain_type = str(Label("@rules_haskell//haskell:toolchain")),
        ))
        content.append("config_setting(\n    name = \"{}_platform\",\n    constraint_values = {},\n)\n".format(
            cell,
            repr(cells[cell].exec_compatible_with),
        ))
        arms.append("        \":{}_platform\": \"@{}//:{{name}}\",".format(cell, cell_repository_name(cell)))
    for name in ["cabal", "ghc_exe", "ghc_files", "gmp", "licenses"]:
        content.append(_HUB_ALIAS.format(name = name, arms = "\n".join(arms).replace("{name}", name)))
    for name in COMMON_LIBRARIES:
        content.append("haskell_toolchain_library(name = \"{}\")\n".format(name))
    for name in os_libraries():
        compatible = [cell for cell in qualified_cells(cells) if name in cells[cell].os_libraries]
        content.append(_OS_LIBRARY.format(name = name, arms = "".join(["        \":{}_platform\": [],\n".format(cell) for cell in compatible]), incompatible = str(Label("@platforms//:incompatible"))))
    cc_toolchains = [
        toolchain_target(
            name = cell,
            exec_compatible_with = cells[cell].exec_compatible_with,
            target_compatible_with = cells[cell].exec_compatible_with,
            target_settings = [str(Label("//haskell/toolchain:ghc_mingw_enabled"))],
            toolchain = "@{}//:cc-compiler-mingw64".format(cell_repository_name(cell)),
            toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
        )
        for cell in qualified_cells(cells)
        if getattr(cells[cell], "mingw_cc", False)
    ]
    return {
        CC_PACKAGE + "/BUILD.bazel": "package(default_visibility = [\"//visibility:public\"])\n\n" + "\n".join(cc_toolchains),
        "BUILD.bazel": "load(\"@rules_haskell//haskell:defs.bzl\", \"haskell_toolchain_library\")\n\npackage(default_visibility = [\"//visibility:public\"])\n\n" + "\n".join(content),
        TOOLCHAINS_PACKAGE + "/BUILD.bazel": "package(default_visibility = [\"//visibility:public\"])\n\n" + "\n".join(toolchains),
    }
