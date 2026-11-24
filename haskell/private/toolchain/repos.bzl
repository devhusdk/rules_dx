"""Hermetic GHC acquisition and toolchain registration."""

load("//core:deb_closure.bzl", "deb_closure")
load("//core:toolchain.bzl", "cell_repo", "pins_hub", "toolchain_target")
load(":pins.bzl", "ALL_CELLS", "CELLS", "CROSS_CELLS", "GHC_VERSION", "MUSL_CELLS", "qualified_cells", "runtime_urls")

HUB_NAME = "dx_haskell_toolchains"

LIBRARIES_PACKAGE = "lib"

CLOSURE_DIR = "dx_closure"

HOST_CLOSURE_DIR = "dx_host_closure"

TARGET_DIR = "_target"

LIBDIR = "lib"

VERSION_FILE = "DX_GHC_VERSION"

def ghc_library_dir(cell):
    """Returns the bindist library directory of one cell."""
    return LIBDIR + "/" + ALL_CELLS[cell].ghc_libdir_triple + "-ghc-" + GHC_VERSION

def ghc_library_dir_name(cell):
    """Returns the base name of one cell's bindist library directory."""
    return ALL_CELLS[cell].ghc_libdir_triple + "-ghc-" + GHC_VERSION

def cross_libdir_moves(host_entries, target_entries, host):
    """Returns the host lib entries a cross cell drops and the target lib entries it takes."""
    keep = ghc_library_dir_name(host)
    return [entry for entry in host_entries if entry != keep], [entry for entry in target_entries if entry != keep]

def cell_matches_host(cell, os_name, arch):
    """Returns whether the cell's binaries run on the host."""
    host = ALL_CELLS[cell].host
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

_TOOLCHAINS_ON = str(Label("//config:toolchains_on"))

_TOOLCHAIN_TYPE = str(Label("//haskell:toolchain_type"))

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

BUNDLED_TOOLS = ["Merge objects command", "windres command", "ar command"]

BUNDLED_FLAGS = ["C compiler flags", "C++ compiler flags", "C compiler link flags"]

MINGW_ROOT = "$topdir/../mingw/"

MINGW_LINK_FLAGS = "-Wl,--no-insert-timestamp"

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

_GHC_BUILD = """load("@rules_cc//cc:cc_import.bzl", "cc_import")
load("@rules_cc//cc:cc_library.bzl", "cc_library")
{cc_loads}load("@rules_haskell//haskell:defs.bzl", "haskell_import", "haskell_toolchain")
{musl_loads}
package(default_visibility = ["//visibility:public"])

exports_files(["{version_file}"])

{libraries}

haskell_toolchain(
    name = "toolchain-impl",
    docdir_path = "doc",
    fully_static_link = {fully_static_link},
    libdir = [":lib"],
    libraries = toolchain_libraries,{llvm}
    locale = "{locale}",
    static_runtime = {static_runtime},
    tools = [":{tools}"],
    version = "{version}",
)

filegroup(
    name = "bin",
    srcs = glob(["bin/*"]),
)
{musl}
filegroup(
    name = "lib",
    srcs = glob(["{libdir}/**", "{closure}/**"{host_closure}]),
)

filegroup(
    name = "cabal",
    srcs = ["cabal/cabal{exe_suffix}"],
)

filegroup(
    name = "ghc_exe",
    {ghc_exe}
)

filegroup(
    name = "ghc_files",
    srcs = [":{tools}", ":lib"],
)

{gmp}

filegroup(
    name = "licenses",
    srcs = glob(["{closure}/licenses/*"]),
)
{cc}"""

_MUSL_LOADS = """load("{musl_bzl}", "musl_tools")
"""

_MUSL_BUILD = """
musl_tools(
    name = "musl_tools",
    srcs = [
        ":bin",
        "{config}",
    ] + glob([
        "{closure}/lib/**",
        "{libdir}/template-hsc.h",
        "{ghclib}/*.so*",
    ]),
    tools = {tools},
)
"""

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
    shared_library = "{closure}/lib/libgmp.so.10",{static}
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

LLVM_TOOLS = {
    "llc": Label("@llvm-project//llvm:llc"),
    "opt": Label("@llvm-project//llvm:opt"),
}

def llvm_toolchain_attrs(llvm_backend):
    """Returns the haskell_toolchain attributes that make GHC compile through llc and opt."""
    if not llvm_backend:
        return ""
    return "".join(["\n    {} = \"{}\",".format(name, tool) for name, tool in sorted(LLVM_TOOLS.items())])

CC_TARGET_FLAGS = ["C compiler flags", "C++ compiler flags"]

def ghc_settings(source, cc_target = ""):
    """Returns the bindist's lib/settings text with every host program replaced; a cc_target sets the C and C++ compiler target."""
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
            if key == "C compiler link flags" and MINGW_ROOT in source:
                bundled = stripped[stripped.index(",") + 1:].strip().rstrip(")").strip().strip("\"")
                line = line[:line.index("(")] + "(" + repr(key) + ", " + repr((bundled + " " + MINGW_LINK_FLAGS).strip()) + ")"
                lines.append(line)
                continue
            if (key in BUNDLED_FLAGS and MINGW_ROOT in source) or (key in BUNDLED_TOOLS and (MINGW_ROOT in source or "\"$tooldir/" in stripped)):
                lines.append(line)
                continue
            value = "--target=" + cc_target if cc_target and key in CC_TARGET_FLAGS else SETTINGS[key]
            line = line[:line.index("(")] + "(" + repr(key) + ", " + repr(value) + ")"
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
            if entry not in value:
                line = field + ": " + entry + " " + value.strip()
            found += 1
        out.append(line)
    if found != 2:
        fail("package registration has no library-dirs and dynamic-library-dirs pair")
    return "\n".join(out)

def links_extra_libraries(conf):
    """Returns whether the package .conf text names any extra libraries."""
    return len([line for line in conf.split("\n") if line.startswith("extra-libraries:")]) > 0

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

MUSL_CONFIG = "dx_ghc_musl.json"

MUSL_TOOL_ARGS = {
    "ghc": ["-B{root}/" + LIBDIR, "-pgmL{bin}/unlit", "-pgmi{bin}/ghc-iserv"],
    "ghc-iserv": [],
    "ghc-iserv-dyn": [],
    "ghc-iserv-prof": [],
    "ghc-pkg": ["--global-package-db={root}/" + LIBDIR + "/package.conf.d"],
    "haddock": ["-B{root}/" + LIBDIR, "--optghc=-pgmL{bin}/unlit", "--optghc=-pgmi{bin}/ghc-iserv"],
    "hp2ps": [],
    "hpc": [],
    "hsc2hs": ["--template={root}/" + LIBDIR + "/template-hsc.h"],
    "runghc": ["-f", "{bin}/ghc"],
    "runhaskell": ["-f", "{bin}/ghc"],
    "unlit": [],
}

HOST_CXX_LIBRARY_DIR = "/usr/lib/gcc/"

def musl_config(cell):
    """Returns the dx ghc_musl config of one musl cell."""
    return {
        "library_path": [CLOSURE_DIR + "/lib", ghc_library_dir(cell)],
        "loader": CLOSURE_DIR + "/lib/" + ALL_CELLS[cell].loader,
        "tools": MUSL_TOOL_ARGS,
    }

def musl_argv(config, root, bin, name):
    """Returns the argv dx ghc_musl runs for one tool of the repository at root."""
    return [
        root + "/" + config["loader"],
        "--library-path",
        ":".join([root + "/" + directory for directory in config["library_path"]]),
        root + "/bin/" + name,
    ] + [arg.replace("{root}", root).replace("{bin}", bin) for arg in config["tools"][name]]

def host_cxx_library_dirs(conf, closure):
    """Returns the package .conf text with the host gcc library dirs replaced by the closure lib directory."""
    out = []
    for line in conf.split("\n"):
        field, sep, value = line.partition(":")
        if sep and HOST_CXX_LIBRARY_DIR in value:
            line = field + ": " + " ".join(["${pkgroot}/../" + closure + "/lib" if word.startswith(HOST_CXX_LIBRARY_DIR) else word for word in value.split(" ") if word])
        out.append(line)
    return "\n".join(out)

def _apk_closure(rctx, cell):
    for package in cell.apks:
        scratch = "_apks/" + package.package
        rctx.download_and_extract(
            url = package.url,
            sha256 = package.sha256,
            output = scratch,
            type = "tar.gz",
        )
        for soname, path in package.libraries.items():
            source = rctx.path(scratch + "/" + path)
            if not source.exists:
                fail("{} does not ship {}".format(package.url, path))
            rctx.rename(source.realpath, CLOSURE_DIR + "/lib/" + soname)
    rctx.delete("_apks")
    for alias, target in cell.loader_aliases.items():
        rctx.symlink(CLOSURE_DIR + "/lib/" + target, CLOSURE_DIR + "/lib/" + alias)

def _musl_config(rctx, cell_name):
    if not rctx.path(LIBDIR + "/template-hsc.h").exists:
        fail("musl bindist ships no {}/template-hsc.h".format(LIBDIR))
    for name in MUSL_TOOL_ARGS:
        if not rctx.path("bin/" + name).exists:
            fail("musl bindist ships no bin/" + name)
    config = musl_config(cell_name)
    rctx.file(MUSL_CONFIG, json.encode(config), executable = False)
    return config

def _cross_libdir(rctx, target):
    rctx.download_and_extract(
        url = target.ghc.urls,
        sha256 = target.ghc.sha256,
        stripPrefix = target.ghc.strip_prefix,
        output = TARGET_DIR,
    )
    dropped, taken = cross_libdir_moves(
        [entry.basename for entry in rctx.path(LIBDIR).readdir()],
        [entry.basename for entry in rctx.path(TARGET_DIR + "/" + LIBDIR).readdir()],
        rctx.attr.cell,
    )
    for name in dropped:
        rctx.delete(LIBDIR + "/" + name)
    for name in taken:
        rctx.rename(TARGET_DIR + "/" + LIBDIR + "/" + name, LIBDIR + "/" + name)
    rctx.delete(TARGET_DIR)

def _runtime_closure(rctx, cell_name, closure):
    packages = ALL_CELLS[cell_name].runtime
    deb_closure(rctx, runtime_urls, packages, closure)
    rctx.symlink(closure + "/lib/libgmp.so.10", closure + "/lib/libgmp.so")
    for package in packages:
        for soname in package.libraries:
            rctx.symlink(closure + "/lib/" + soname, ghc_library_dir(cell_name) + "/" + soname)

def _ghc_repo_impl(rctx):
    host = ALL_CELLS[rctx.attr.cell]
    target_name = rctx.attr.target or rctx.attr.cell
    cell = ALL_CELLS[target_name]
    if not cell_matches_host(rctx.attr.cell, rctx.os.name, rctx.os.arch):
        rctx.file("REPO.bazel", "")
        rctx.file("BUILD.bazel", _UNAVAILABLE_BUILD)
        return rctx.repo_metadata(reproducible = True)
    rctx.download_and_extract(
        url = host.ghc.urls,
        sha256 = host.ghc.sha256,
        stripPrefix = host.ghc.strip_prefix,
    )
    rctx.download_and_extract(
        url = host.cabal.urls,
        sha256 = host.cabal.sha256,
        output = "cabal",
    )
    for path in GHC_DROPPED_PATHS:
        rctx.delete(path)
    if rctx.attr.target:
        _cross_libdir(rctx, cell)
        if host.runtime:
            _runtime_closure(rctx, rctx.attr.cell, HOST_CLOSURE_DIR)
    apks = getattr(cell, "apks", [])
    if apks:
        _apk_closure(rctx, cell)
    if cell.runtime:
        _runtime_closure(rctx, target_name, CLOSURE_DIR)
    rctx.file(LIBDIR + "/settings", ghc_settings(rctx.read(LIBDIR + "/settings"), rctx.attr.cc_target), executable = False)
    if cell.runtime or apks:
        _gmp_registration(rctx)
    if getattr(cell, "gmp_license", False):
        rctx.file(CLOSURE_DIR + "/licenses/gmp", rctx.read(rctx.attr._gmp_license), executable = False)
    if host.exe_suffix == ".exe":
        _drop_import_libs(rctx)
    for entry in rctx.path(LIBDIR + "/package.conf.d").readdir():
        if entry.basename.endswith(".conf"):
            conf = rctx.read(entry)
            stripped = host_cxx_library_dirs(drop_host_sdk_dirs(conf), CLOSURE_DIR)
            if apks and links_extra_libraries(stripped):
                stripped = closure_library_dirs(stripped, CLOSURE_DIR)
            if stripped != conf:
                rctx.file(entry, stripped, executable = False)
    if apks:
        config = _musl_config(rctx, rctx.attr.cell)
        bin_dir = rctx.path("bin")
        ghc_pkg = musl_argv(config, str(bin_dir.dirname), str(bin_dir), "ghc-pkg")
        ghc = musl_argv(config, str(bin_dir.dirname), str(bin_dir), "ghc")
    else:
        ghc_pkg = ["bin/ghc-pkg" + host.exe_suffix]
        ghc = ["bin/ghc" + host.exe_suffix]
    _execute(rctx, ghc_pkg + ["recache", "--global"])
    version = _execute(rctx, ghc + ["--numeric-version"]).stdout.strip()
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
        exe_suffix = host.exe_suffix,
        ghc_exe = "srcs = [\":musl_tools\"],\n    output_group = \"ghc\"," if apks else "srcs = [\"bin/ghc{}\"],".format(host.exe_suffix),
        host_closure = ", \"{}/**\"".format(HOST_CLOSURE_DIR) if rctx.attr.target and host.runtime else "",
        gmp = (_GMP_IMPORT if cell.runtime or apks else _GMP_EMPTY).format(
            closure = CLOSURE_DIR,
            static = "\n    static_library = \"{}/lib/libgmp.a\",".format(CLOSURE_DIR) if apks else "",
        ),
        libdir = LIBDIR,
        libraries = result["file_content"],
        llvm = llvm_toolchain_attrs(rctx.attr.llvm_backend),
        locale = host.locale,
        musl = _MUSL_BUILD.format(
            closure = CLOSURE_DIR,
            config = MUSL_CONFIG,
            ghclib = ghc_library_dir(rctx.attr.cell),
            libdir = LIBDIR,
            tools = repr(sorted(MUSL_TOOL_ARGS.keys())),
        ) if apks else "",
        musl_loads = _MUSL_LOADS.format(musl_bzl = str(Label("//haskell/private/toolchain:musl.bzl"))) if apks else "",
        tools = "musl_tools" if apks else "bin",
        fully_static_link = bool(apks),
        static_runtime = cell.exe_suffix == ".exe",
        version = GHC_VERSION,
        version_file = VERSION_FILE,
    ))
    return rctx.repo_metadata(reproducible = True)

ghc_repo = repository_rule(
    implementation = _ghc_repo_impl,
    doc = "Acquires one relocatable GHC bindist plus its native runtime closure; with `target`, the host bindist's executables over the target bindist's libraries.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
        "target": attr.string(doc = "Qualified target cell of a cross toolchain."),
        "cc_target": attr.string(doc = "C compiler target triple of a cross toolchain."),
        "llvm_backend": attr.bool(doc = "Whether GHC compiles through llc and opt instead of its native code generator."),
        "_pkgdb_to_bzl": attr.label(default = Label("@rules_haskell//haskell:private/pkgdb_to_bzl.py")),
        "_gmp_license": attr.label(default = Label("//haskell/private/toolchain:lgpl-3.0.txt")),
        "python": attr.label(mandatory = True, doc = "Host interpreter for the package database conversion."),
    },
)

def alias_repository(cell, host_os = None, host_arch = None):
    """Returns the repository the hub aliases of one target cell select: the cross cell when only that one runs on the host."""
    if host_os == None or cell_matches_host(cell, host_os, host_arch):
        return cell_repo(HUB_NAME, cell)
    for name, cross in sorted(CROSS_CELLS.items()):
        if cross.target == cell and cell_matches_host(cross.host, host_os, host_arch):
            return cell_repo(HUB_NAME, name)
    return cell_repo(HUB_NAME, cell)

def hub_files(cells = CELLS, musl_cells = MUSL_CELLS, host_os = None, host_arch = None):
    """Returns the hub files: the toolchains and cc toolchains at the root, the runtime closure aliases under lib."""
    content = []
    arms = []
    for cell in qualified_cells(musl_cells) + qualified_cells(cells):
        info = musl_cells[cell] if cell in musl_cells else cells[cell]
        target = getattr(info, "target_compatible_with", info.exec_compatible_with)
        content.append("config_setting(\n    name = \"{}_platform\",\n    constraint_values = {},\n)\n".format(
            cell,
            repr(target),
        ))
        arms.append("        \":{}_platform\": \"@{}//:{{name}}\",".format(cell, alias_repository(cell, host_os, host_arch)))
    for name in ["cabal", "ghc_exe", "ghc_files", "gmp", "licenses"]:
        content.append(_HUB_ALIAS.format(name = name, arms = "\n".join(arms).replace("{name}", name)))
    for name in COMMON_LIBRARIES:
        content.append("haskell_toolchain_library(name = \"{}\")\n".format(name))
    for name in os_libraries():
        compatible = [cell for cell in qualified_cells(musl_cells) if name in musl_cells[cell].os_libraries] + [cell for cell in qualified_cells(cells) if name in cells[cell].os_libraries]
        content.append(_OS_LIBRARY.format(name = name, arms = "".join(["        \":{}_platform\": [],\n".format(cell) for cell in compatible]), incompatible = str(Label("@platforms//:incompatible"))))
    hub = dict(compatible_with = "both", target = "toolchain-impl", target_settings = [_TOOLCHAINS_ON])
    toolchains = [
        pins_hub(HUB_NAME, qualified_cells(musl_cells), _TOOLCHAIN_TYPE, cell_constraints = musl_cells, **hub),
        pins_hub(HUB_NAME, qualified_cells(cells), _TOOLCHAIN_TYPE, **hub),
        pins_hub(HUB_NAME, CROSS_CELLS.keys(), _TOOLCHAIN_TYPE, cell_constraints = {
            name: struct(exec_compatible_with = cells[cross.host].exec_compatible_with, target_compatible_with = cells[cross.target].exec_compatible_with)
            for name, cross in CROSS_CELLS.items()
        }, **hub),
    ]
    cc_toolchains = [
        toolchain_target(
            name = "cc_" + cell,
            exec_compatible_with = cells[cell].exec_compatible_with,
            target_compatible_with = cells[cell].exec_compatible_with,
            target_settings = [_TOOLCHAINS_ON, str(Label("//haskell/private/toolchain:ghc_mingw_enabled"))],
            toolchain = "@{}//:cc-compiler-mingw64".format(cell_repo(HUB_NAME, cell)),
            toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
        )
        for cell in sorted(qualified_cells(cells))
        if getattr(cells[cell], "mingw_cc", False)
    ]
    return {
        "BUILD.bazel": "package(default_visibility = [\"//visibility:public\"])\n\n" + "\n".join(cc_toolchains + toolchains),
        LIBRARIES_PACKAGE + "/BUILD.bazel": "load(\"@rules_haskell//haskell:defs.bzl\", \"haskell_toolchain_library\")\n\npackage(default_visibility = [\"//visibility:public\"])\n\n" + "\n".join(content),
    }
