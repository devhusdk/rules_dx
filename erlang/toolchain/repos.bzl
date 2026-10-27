"""Hermetic Erlang/OTP acquisition and toolchain registration."""

load(
    ":pins.bzl",
    "CELLS",
    "DROPPED_APPLICATIONS",
    "DROPPED_APPLICATION_DIRS",
    "DROPPED_PATHS",
    "ERLFMT_VERSION",
    "ERTS_VERSION",
    "GETOPT_VERSION",
    "OTP_RELEASE",
    "OTP_VERSION",
    "RULES_ERLANG_COMMIT",
    "RULES_ERLANG_SHA256",
    "RULES_ERLANG_URL",
    "RUNTIME_MIRROR",
    "TOOL_SOURCES",
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

_OTP_BUILD = """load("@rules_dx//erlang/toolchain:otp.bzl", "dx_otp")
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
"""

_TOOLCHAIN = """toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:erlang",
    toolchain_type = "@rules_erlang//tools:toolchain_type",
)
"""

_GETOPT_BUILD = """load("@rules_erlang//:erlang_app.bzl", "erlang_app")

package(default_visibility = ["//visibility:public"])

erlang_app(
    app_name = "getopt",
    app_version = "{version}",
)
"""

_ERLFMT_BUILD = """load("@rules_dx//erlang/rules:defs.bzl", "ERLANG_COMPATIBLE_WITH", "escript_archive")
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

escript_archive(
    name = "erlfmt",
    app = ":erlang_app",
    target_compatible_with = ERLANG_COMPATIBLE_WITH,
    headers = [
        "shebang",
        "{{emu_args, \\"+sbtu +A0 -noinput -noshell -mode minimal -escript main erlfmt\\"}}",
    ],
)
"""

TOOL_BUILDS = {
    "dx_erlang_getopt": _GETOPT_BUILD.format(version = GETOPT_VERSION),
    "dx_erlfmt": _ERLFMT_BUILD.format(version = ERLFMT_VERSION),
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

def _rules_erlang_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_erlang-" + rctx.attr.commit,
    )
    rctx.delete("MODULE.bazel")
    rctx.delete("gazelle")
    rctx.delete("test")
    rctx.file("BUILD.bazel", _ROOT_BUILD)
    rctx.file("private/BUILD.bazel", _PRIVATE_BUILD)
    rctx.file("REPO.bazel", "")
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

_rules_erlang = repository_rule(
    implementation = _rules_erlang_impl,
    doc = "Fetches the pinned rules_erlang sources with the hermetic OTP patch.",
    attrs = {
        "commit": attr.string(mandatory = True, doc = "Pinned rules_erlang commit."),
        "patches": attr.label_list(doc = "Patches applied after extraction."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
    },
)

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

def _prune(rctx):
    for path in DROPPED_PATHS:
        rctx.delete(path)
    for app in rctx.path("lib").readdir():
        if dropped_application(app.basename):
            rctx.delete(app)
            continue
        for sub in DROPPED_APPLICATION_DIRS:
            rctx.delete(app.get_child(sub))

def _closure(rctx, packages):
    for package in packages:
        scratch = "_debs/" + package.package
        rctx.download_and_extract(
            url = RUNTIME_MIRROR + "/" + package.deb,
            sha256 = package.sha256,
            output = scratch,
            type = "deb",
        )
        data = [entry for entry in rctx.path(scratch).readdir() if entry.basename.startswith("data.tar")]
        if len(data) != 1:
            fail("{} has no single data archive".format(package.deb))
        rctx.extract(data[0], output = scratch + "/data")
        for soname, path in package.libraries.items():
            source = rctx.path(scratch + "/data/" + path)
            if not source.exists:
                fail("{} does not ship {}".format(package.deb, path))
            rctx.rename(source.realpath, CLOSURE_DIR + "/lib/" + soname)
        if package.copyright != None:
            copyright = rctx.path(scratch + "/data/" + package.copyright)
            if not copyright.exists:
                fail("{} does not ship {}".format(package.deb, package.copyright))
            rctx.file(CLOSURE_DIR + "/licenses/" + package.package, rctx.read(copyright), executable = False)
    rctx.delete("_debs")

def _otp_repo_impl(rctx):
    cell = CELLS[rctx.attr.cell]
    rctx.download_and_extract(
        url = cell.otp_url,
        sha256 = cell.otp_sha256,
        stripPrefix = cell.strip_prefix,
    )
    if rctx.read("releases/" + OTP_RELEASE + "/OTP_VERSION").strip() != OTP_VERSION:
        fail("OTP archive is not release " + OTP_VERSION)
    _prune(rctx)
    _install(rctx, CLOSURE_DIR if cell.runtime else None)
    if cell.runtime:
        _closure(rctx, cell.runtime)
    rctx.file(VERSION_FILE, OTP_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _OTP_BUILD.format(
        closure = CLOSURE_DIR,
        version = OTP_VERSION,
        version_file = VERSION_FILE,
    ))
    return rctx.repo_metadata(reproducible = True)

_otp_repo = repository_rule(
    implementation = _otp_repo_impl,
    doc = "Acquires one relocatable OTP tree plus its native runtime closure.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Qualified host cell."),
    },
)

def _tool_source_impl(rctx):
    source = TOOL_SOURCES[rctx.attr.tool]
    rctx.download_and_extract(
        url = source.url,
        sha256 = source.sha256,
        stripPrefix = source.strip_prefix,
    )
    for path in source.dropped:
        rctx.delete(path)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", TOOL_BUILDS[rctx.attr.tool])
    return rctx.repo_metadata(reproducible = True)

_tool_source = repository_rule(
    implementation = _tool_source_impl,
    doc = "Fetches the pinned sources of one Erlang quality tool.",
    attrs = {
        "tool": attr.string(mandatory = True, doc = "Pinned tool source name."),
    },
)

def _hub_impl(rctx):
    content = []
    for cell in sorted(CELLS.keys()):
        content.append(_TOOLCHAIN.format(
            cell = cell,
            constraints = repr(CELLS[cell].exec_compatible_with),
            repo = cell_repository_name(cell),
        ))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares one Erlang toolchain per qualified host cell.",
)

def _erlang_impl(mctx):
    _rules_erlang(
        name = "rules_erlang",
        commit = RULES_ERLANG_COMMIT,
        patches = [Label("//patches/rules_erlang:hermetic_otp.patch")],
        sha256 = RULES_ERLANG_SHA256,
        url = RULES_ERLANG_URL,
    )
    for cell in sorted(CELLS.keys()):
        _otp_repo(name = cell_repository_name(cell), cell = cell)
    _hub(name = HUB_NAME)
    for tool in sorted(TOOL_SOURCES.keys()):
        _tool_source(name = tool, tool = tool)
    return mctx.extension_metadata(reproducible = True)

erlang = module_extension(
    implementation = _erlang_impl,
    doc = "Creates rules_erlang, the pinned OTP trees, the toolchain hub and the erlfmt sources.",
)
