"""Hermetic Erlang/OTP acquisition and toolchain registration."""

load("//elixir/rules:hex.bzl", "hex_package")
load("//libs/starlark:deb_closure.bzl", "deb_closure")
load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load("//libs/starlark:toolchain.bzl", "toolchain_target")
load(
    ":pins.bzl",
    "CELLS",
    "DROPPED_APPLICATIONS",
    "DROPPED_APPLICATION_DIRS",
    "DROPPED_PATHS",
    "ELVIS_CORE_VERSION",
    "ELVIS_VERSION",
    "ERLFMT_VERSION",
    "ERTS_VERSION",
    "GETOPT_VERSION",
    "HEX_SOURCES",
    "KATANA_CODE_VERSION",
    "OTP_RELEASE",
    "OTP_VERSION",
    "RULES_ERLANG_COMMIT",
    "RULES_ERLANG_SHA256",
    "RULES_ERLANG_URL",
    "RUNTIME_MIRROR",
    "TOOL_SOURCES",
    "WINDOWS_DROPPED_PATHS",
)

HUB_NAME = "dx_erlang_toolchains"

CLOSURE_DIR = "dx_closure"

VERSION_FILE = "DX_OTP_VERSION"

BIN_TOOLS = ["ct_run", "dialyzer", "erl_call", "erlc", "escript", "typer"]

BOOT_FILES = ["no_dot_erlang.boot", "start_clean.boot", "start_sasl.boot"]

_CELL_PREFIX = "dx_otp_"

_ROOT_BUILD = """load("@bazel_skylib//rules:common_settings.bzl", "string_flag")

package(default_visibility = ["//visibility:public"])

config_setting(
    name = "debug_build",
    values = {"compilation_mode": "dbg"},
)

string_flag(name = "erlang_home", build_setting_default = "")

string_flag(name = "erlang_version", build_setting_default = "")

string_flag(name = "ct_logdir", build_setting_default = "")

string_flag(name = "ct_test_windows_logdir_drive_letter", build_setting_default = "b")

exports_files(["LICENSE", "LICENSE-APACHE2", "LICENSE-MPL-RabbitMQ"])
"""

_PRIVATE_BUILD = """config_setting(
    name = "private_stamp_detect",
    values = {"stamp": "1"},
)
"""

_OTP_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_library")
load("@rules_dx//erlang/toolchain:otp.bzl", "dx_otp")
load("@rules_erlang//tools:erlang_toolchain.bzl", "erlang_toolchain")

package(default_visibility = ["//visibility:public"])

dx_otp(
    name = "otp",
    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel"]),
    root = "{version_file}",
    version = "{version}",
)

erlang_toolchain(
    name = "erlang",
    otp = ":otp",
)

cc_library(
    name = "erl_nif",
    hdrs = glob(["{include}/*.h"], allow_empty = False),
    includes = ["{include}"],
)
"""

_HUB_NIF = """cc_library(
    name = "erl_nif",
    target_compatible_with = select({compatible}),
    deps = select({deps}),
)
"""

def nif_include_dir():
    """Returns the OTP-relative directory that holds erl_nif.h."""
    return "erts-" + ERTS_VERSION + "/include"

def erts_mismatch_message(expected, found_dirs):
    """Returns the error for an OTP archive whose erts directory differs from the ERTS_VERSION pin."""
    found = ", ".join(sorted(found_dirs)) if found_dirs else "none"
    return (
        "OTP archive ships no erts-" + expected + " directory (found: " + found + "); " +
        "set ERTS_VERSION in erlang/toolchain/pins.bzl to match the erts-* directory of OTP " + OTP_VERSION
    )

def nif_header_cases(cells):
    """Returns the hub select cases that map each target cell to its erl_nif header library."""
    cases = {"@rules_dx//erlang/toolchain:cell_" + cell: ["@" + cell_repository_name(cell) + "//:erl_nif"] for cell in sorted(cells)}
    cases["//conditions:default"] = []
    return cases

def nif_compatible_cases(cells):
    """Returns the hub select cases that make erl_nif incompatible outside the pinned cells."""
    cases = {"@rules_dx//erlang/toolchain:cell_" + cell: [] for cell in sorted(cells)}
    cases["//conditions:default"] = ["@platforms//:incompatible"]
    return cases

_GETOPT_BUILD = """load("@rules_erlang//:erlang_app.bzl", "erlang_app")

package(default_visibility = ["//visibility:public"])

erlang_app(
    app_name = "getopt",
    app_version = "{version}",
)
"""

_ERLFMT_BUILD = """load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH", "erlang_escript_archive")
load("@rules_erlang//:erlang_app.bzl", "erlang_app")
load("@rules_erlang//:erlang_xrl_yrl.bzl", "erlang_xrl_yrl")

package(default_visibility = ["//visibility:public"])

erlang_xrl_yrl(
    name = "parser",
    srcs = ["src/erlfmt_parse.yrl"],
    outs = ["src/erlfmt_parse.erl"],
)

erlang_app(
    app_name = "erlfmt",
    app_version = "{version}",
    extra_srcs = ["src/erlfmt_parse.erl"],
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    deps = ["@dx_erlang_getopt//:erlang_app"],
)

erlang_escript_archive(
    name = "erlfmt",
    app = ":erlang_app",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    headers = [
        "shebang",
        "{{emu_args, \\"+sbtu +A0 -noinput -noshell -mode minimal -escript main erlfmt\\"}}",
    ],
)
"""

_KATANA_CODE_BUILD = """load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH")
load("@rules_erlang//:erlang_app.bzl", "erlang_app")

package(default_visibility = ["//visibility:public"])

erlang_app(
    app_name = "katana_code",
    app_version = "{version}",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
)
"""

_ELVIS_CORE_BUILD = """load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH")
load("@rules_erlang//:erlang_app.bzl", "erlang_app")

package(default_visibility = ["//visibility:public"])

erlang_app(
    app_name = "elvis_core",
    app_version = "{version}",
    extra_apps = ["syntax_tools"],
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    deps = ["@dx_katana_code//:erlang_app"],
)
"""

_ELVIS_BUILD = """load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH", "erlang_escript_archive")
load("@rules_erlang//:erlang_app.bzl", "erlang_app")

package(default_visibility = ["//visibility:public"])

erlang_app(
    app_name = "elvis",
    app_version = "{version}",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    deps = [
        "@dx_elvis_core//:erlang_app",
        "@dx_erlang_getopt//:erlang_app",
    ],
)

erlang_escript_archive(
    name = "elvis",
    app = ":erlang_app",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    headers = [
        "shebang",
        "{{emu_args, \\"+sbtu +A0 -noinput -noshell -mode minimal -escript main elvis\\"}}",
    ],
)
"""

_HEX_BUILD = """load("@rules_erlang//:erlang_app.bzl", "erlang_app")

package(default_visibility = ["//visibility:public"])

erlang_app(
    app_name = "{app}",
    app_version = "{version}",
    deps = {deps},
)
"""

HEX_DEPS = {"dx_erlperf": ["@dx_erlang_argparse//:erlang_app"]}

TOOL_BUILDS = {
    "dx_elvis": _ELVIS_BUILD.format(version = ELVIS_VERSION),
    "dx_elvis_core": _ELVIS_CORE_BUILD.format(version = ELVIS_CORE_VERSION),
    "dx_erlang_getopt": _GETOPT_BUILD.format(version = GETOPT_VERSION),
    "dx_erlfmt": _ERLFMT_BUILD.format(version = ERLFMT_VERSION),
    "dx_katana_code": _KATANA_CODE_BUILD.format(version = KATANA_CODE_VERSION),
}

def cell_repository_name(cell):
    """Returns the repository name one qualified cell is acquired into."""
    return _CELL_PREFIX + cell

RESTORE_VARIABLE = "DX_ERL_LOADER_RESTORE"

RESTORE_EVAL = 'case os:getenv("' + RESTORE_VARIABLE + '") of false -> ok; "" -> os:unsetenv("LD_LIBRARY_PATH"); Saved -> os:putenv("LD_LIBRARY_PATH", Saved) end.'

def erl_script(source, closure):
    """Returns the relocatable erl launcher whose loader path covers only the emulator."""
    marker = "export PROGNAME\n"
    launch = 'exec "$BINDIR/erlexec" ${1+"$@"}'
    if marker not in source:
        fail("erl.src no longer exports PROGNAME")
    if launch not in source:
        fail("erl.src no longer execs erlexec")
    relocated = source.replace("%FINAL_ROOTDIR%", "/nonexistent")
    if not closure:
        return relocated
    loader = RESTORE_VARIABLE + '="${LD_LIBRARY_PATH-}"\nexport ' + RESTORE_VARIABLE + "\n"
    loader += 'LD_LIBRARY_PATH="$ROOTDIR/' + closure + '/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\nexport LD_LIBRARY_PATH\n'
    restored = 'exec "$BINDIR/erlexec" -eval \'' + RESTORE_EVAL + '\' ${1+"$@"}'
    return relocated.replace(marker, marker + loader).replace(launch, restored)

def dropped_application(directory):
    """Returns whether one lib directory is an application the runtime drops."""
    return directory.split("-")[0] in DROPPED_APPLICATIONS

def _link(rctx, target, link):
    rctx.delete(link)
    rctx.symlink(target, link)

def _install(rctx, closure):
    erts_bin = "erts-" + ERTS_VERSION + "/bin/"
    erl = erl_script(rctx.read(erts_bin + "erl.src"), closure)
    rctx.file(erts_bin + "erl", erl, executable = True)
    rctx.file("bin/erl", erl, executable = True)
    for tool in BIN_TOOLS:
        _link(rctx, erts_bin + tool, "bin/" + tool)
    release = "releases/" + OTP_RELEASE + "/"
    for boot in BOOT_FILES:
        _link(rctx, release + boot, "bin/" + boot)
    _link(rctx, release + "start_clean.boot", "bin/start.boot")
    _link(rctx, release + "start_clean.script", "bin/start.script")
    rctx.file("releases/start_erl.data", ERTS_VERSION + " " + OTP_RELEASE + "\n")
    rctx.file("releases/RELEASES", rctx.read("releases/RELEASES.src").replace("%ERL_ROOT%/", ""))

def is_windows_cell(cell):
    """Returns whether a pinned cell runs on Windows."""
    return "@platforms//os:windows" in cell.exec_compatible_with

def _prune(rctx, windows):
    for path in DROPPED_PATHS + (WINDOWS_DROPPED_PATHS if windows else []):
        rctx.delete(path)
    for app in rctx.path("lib").readdir():
        if dropped_application(app.basename):
            rctx.delete(app)
            continue
        for sub in DROPPED_APPLICATION_DIRS:
            rctx.delete(app.get_child(sub))

def _otp_repo_impl(rctx):
    cell = CELLS[rctx.attr.cell]
    rctx.download_and_extract(
        url = cell.otp_url,
        sha256 = cell.otp_sha256,
        stripPrefix = cell.strip_prefix,
    )
    if rctx.read("releases/" + OTP_RELEASE + "/OTP_VERSION").strip() != OTP_VERSION:
        fail("OTP archive is not release " + OTP_VERSION)
    windows = is_windows_cell(cell)
    _prune(rctx, windows)
    if not rctx.path(nif_include_dir() + "/erl_nif.h").exists:
        found = [p.basename for p in rctx.path(".").readdir() if p.basename.startswith("erts-")]
        if "erts-" + ERTS_VERSION not in found:
            fail(erts_mismatch_message(ERTS_VERSION, found))
        fail("OTP archive ships no " + nif_include_dir() + "/erl_nif.h")
    if windows:
        if not rctx.path("bin/erl.exe").exists:
            fail("OTP archive ships no bin/erl.exe")
    else:
        _install(rctx, CLOSURE_DIR if cell.runtime else None)
    if cell.runtime:
        deb_closure(rctx, RUNTIME_MIRROR, cell.runtime, CLOSURE_DIR)
    rctx.file(VERSION_FILE, OTP_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _OTP_BUILD.format(
        closure = CLOSURE_DIR,
        version = OTP_VERSION,
        version_file = VERSION_FILE,
        include = nif_include_dir(),
    ))
    return rctx.repo_metadata(reproducible = True)

_otp_repo = repository_rule(
    implementation = _otp_repo_impl,
    doc = "Acquires one relocatable OTP tree plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
    },
)

def hub_build():
    """Returns the hub BUILD file: one Erlang toolchain per qualified cell and the erl_nif alias."""
    content = ['load("@rules_cc//cc:defs.bzl", "cc_library")\n\npackage(default_visibility = ["//visibility:public"])\n']
    for cell in sorted(CELLS.keys()):
        content.append(toolchain_target(
            name = cell,
            exec_compatible_with = CELLS[cell].exec_compatible_with,
            target_compatible_with = CELLS[cell].exec_compatible_with,
            toolchain = "@{}//:erlang".format(cell_repository_name(cell)),
            toolchain_type = str(Label("@rules_erlang//tools:toolchain_type")),
        ))
    content.append(_HUB_NIF.format(
        compatible = repr(nif_compatible_cases(CELLS.keys())),
        deps = repr(nif_header_cases(CELLS.keys())),
    ))
    return "\n".join(content)

def _erlang_impl(mctx):
    pinned_archive(
        name = "rules_erlang",
        dropped = ["MODULE.bazel", "gazelle", "test"],
        files = {
            "BUILD.bazel": _ROOT_BUILD,
            "private/BUILD.bazel": _PRIVATE_BUILD,
        },
        patches = [
            Label("//patches/rules_erlang:01_xrl_yrl.patch"),
            Label("//patches/rules_erlang:02_tcw_tags.patch"),
            Label("//patches/rules_erlang:03_auto_sname.patch"),
            Label("//patches/rules_erlang:04_otp_tree.patch"),
        ],
        sha256 = RULES_ERLANG_SHA256,
        strip_prefix = "rules_erlang-" + RULES_ERLANG_COMMIT,
        type = "tar.gz",
        urls = [RULES_ERLANG_URL],
    )
    for cell in sorted(CELLS.keys()):
        _otp_repo(name = cell_repository_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    for tool in sorted(TOOL_SOURCES.keys()):
        source = TOOL_SOURCES[tool]
        pinned_archive(
            name = tool,
            dropped = source.dropped,
            files = {"BUILD.bazel": TOOL_BUILDS[tool]},
            patches = source.patches,
            sha256 = source.sha256,
            strip_prefix = source.strip_prefix,
            urls = [source.url],
        )
    for name, hex in sorted(HEX_SOURCES.items()):
        hex_package(
            name = name,
            build_file_content = _HEX_BUILD.format(app = hex.package, version = hex.version, deps = repr(HEX_DEPS.get(name, []))),
            package = hex.package,
            sha256 = hex.sha256,
            version = hex.version,
        )
    return mctx.extension_metadata(reproducible = True)

erlang = module_extension(
    implementation = _erlang_impl,
    doc = "Creates rules_erlang, the pinned OTP trees, the toolchain hub and the erlfmt and elvis sources.",
)
