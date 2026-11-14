"""Elixir toolchain pin and launcher tests."""

load("//elixir/private/tools:launcher.bzl", "MIX_FORMAT_ENV", "elixir_descriptor")
load("//erlang/private/toolchain:pins.bzl", "CELLS", "OTP_VERSION")
load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "starlark_test")
load("//elixir/private/toolchain:pins.bzl", "CREDO_PACKAGES", "ELIXIR_CELLS", "ELIXIR_OTP_MAJOR")
load("//elixir/private/toolchain:repos.bzl", "CREDO_REWRITES", "DROPPED_PATHS", "credo_build", "credo_rewrites", "elixir_bat", "elixir_script", "hub_build")

_BAT_SOURCE = """if x (
  echo "%ERTS_BIN%erl.exe" %ELIXIR_ERL_OPTIONS% %*
) else (
  "%ERTS_BIN%erl.exe" %ELIXIR_ERL_OPTIONS% %*
)
"""

_SOURCE = "#" + """!/bin/sh
if [ "$OSTYPE" = "cygwin" ]; then SCRIPT_PATH=$(cygpath -m "$SCRIPT_PATH"); fi
set -- "$ERTS_BIN$ERL_EXEC" -noshell -pa "$SCRIPT_PATH"/../lib/elixir/ebin $ELIXIR_ERL_OPTIONS $ERL "$@"
"""

_ERL_ARGS = "arg=-noshell\narg=-elixir_root\npath=elixir/lib\narg=-pa\npath=elixir/lib/elixir/ebin\n"

_WINDOWS_DESCRIPTOR = "program=otp/bin/erl.exe\n" + _ERL_ARGS + """arg=-pa
path=credo/ebin
arg=+pc
arg=unicode
arg=-s
arg=elixir
arg=start_cli
arg=-extra
path=elixir/bin/mix
arg=format
env_dir=PATH\totp/bin/erl.exe
env=ELIXIR_ERL_OPTIONS\t-elixir ansi_enabled false
env=HEX_HOME\tC:/dx_nonexistent/dx_mix_format/hex
env=HEX_OFFLINE\t1
env=HOME\tC:/dx_nonexistent/dx_mix_format
env=MIX_ARCHIVES\tC:/dx_nonexistent/dx_mix_format/archives
env=MIX_ENV\tdev
env=MIX_HOME\tC:/dx_nonexistent/dx_mix_format
"""

_WINDOWS_CALLER_HOMES_DESCRIPTOR = "program=otp/bin/erl.exe\n" + _ERL_ARGS + """arg=+pc
arg=unicode
arg=-s
arg=elixir
arg=start_cli
arg=-extra
path=elixir/bin/mix
env_dir=PATH\totp/bin/erl.exe
env_default=HEX_HOME\tC:/dx_nonexistent/dx_mix_format/hex
env_default=HOME\tC:/dx_nonexistent/dx_mix_format
env_default=MIX_HOME\tC:/dx_nonexistent/dx_mix_format
env=ELIXIR_ERL_OPTIONS\t-elixir ansi_enabled false
"""

_UNIX_CALLER_HOMES_DESCRIPTOR = "program=otp/bin/erl\n" + _ERL_ARGS + """arg=+pc
arg=unicode
arg=+fnu
arg=-kernel
arg=standard_io_encoding
arg=unicode
arg=-s
arg=elixir
arg=start_cli
arg=-extra
path=elixir/bin/mix
env_dir=ERL_ROOTDIR\totp/VERSION
env_default=HEX_HOME\t/nonexistent/dx_mix_format/hex
env_default=HOME\t/nonexistent/dx_mix_format
env_default=MIX_HOME\t/nonexistent/dx_mix_format
env=ELIXIR_ERL_OPTIONS\t-elixir ansi_enabled false
"""

def _path_lines(descriptor):
    return [line for line in descriptor.split("\n") if line.partition("=")[2].startswith("PATH\t")]

def _launcher_checks():
    windows = elixir_descriptor("otp/VERSION", "elixir", ["credo/ebin"], "elixir/bin/mix", True, ["format"])
    windows_caller = elixir_descriptor("otp/VERSION", "elixir", [], "elixir/bin/mix", True, [], True)
    unix = elixir_descriptor("otp/VERSION", "elixir", [], "elixir/bin/mix", False)
    unix_caller = elixir_descriptor("otp/VERSION", "elixir", [], "elixir/bin/mix", False, [], True)
    return [
        expect_equal("windows launcher descriptor", windows, _WINDOWS_DESCRIPTOR),
        expect_equal("windows caller_homes launcher descriptor", windows_caller, _WINDOWS_CALLER_HOMES_DESCRIPTOR),
        expect_equal("unix caller_homes launcher descriptor", unix_caller, _UNIX_CALLER_HOMES_DESCRIPTOR),
        expect_equal("windows PATH is the OTP bin directory of the runfiles", _path_lines(windows) + _path_lines(windows_caller), ["env_dir=PATH\totp/bin/erl.exe"] * 2),
        expect_equal("unix launchers set no PATH", _path_lines(unix) + _path_lines(unix_caller), []),
        expect_equal("no launcher names a Windows system directory", [d for d in [windows, windows_caller, unix, unix_caller] if "Windows" in d or "System32" in d], []),
    ]

def _credo_checks():
    descriptor = elixir_descriptor("otp/VERSION", "elixir", [], "elixir/bin/mix", False)
    return [
        expect_equal("every rewritten package is pinned", [name for name in CREDO_REWRITES if name not in CREDO_PACKAGES], []),
        expect_equal("a marker present once is rewritten", credo_rewrites("credo", "lib/credo/cli/output/ui.ex", "if Mix.env() == :test do", "1"), "if false do"),
        expect_equal("the version is pinned to the literal", credo_rewrites("bunt", "lib/bunt.ex", "@version Mix.Project.config()[:version]", "1.0.0"), '@version "1.0.0"'),
        expect_equal("other files pass through", credo_rewrites("credo", "lib/other.ex", "Mix.env()", "1"), "Mix.env()"),
        expect_contains("credo builds with the dependency chain", credo_build("credo"), '"//file_system:erlang_app"'),
        expect_contains("vendored apps do not fail on warnings", credo_build("jason"), "warnings_as_errors = False"),
        expect_contains("launcher locks the home down", descriptor, "env=HOME\t/nonexistent/dx_mix_format"),
        expect_contains("launcher roots OTP at its tree", descriptor, "env_dir=ERL_ROOTDIR\totp/VERSION"),
        expect_equal("launcher env keeps its name", MIX_FORMAT_ENV["HEX_OFFLINE"], "1"),
    ]

def _checks():
    script = elixir_script(_SOURCE)
    hub = hub_build(ELIXIR_CELLS)
    return [
        expect_equal("build matches the OTP major", ELIXIR_OTP_MAJOR, OTP_VERSION.split(".")[0]),
        expect_contains("launcher forces Unicode names and stdio", script, "+fnu -kernel standard_io_encoding unicode $ELIXIR_ERL_OPTIONS $ERL"),
        expect_equal("elixir runs only on qualified cells", ELIXIR_CELLS, ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]),
        expect_equal("the hub registers each Elixir cell", hub.count("toolchain("), len(ELIXIR_CELLS)),
        expect_equal("the hub skips other OTP cells", [cell for cell in CELLS if cell not in ELIXIR_CELLS and '"' + cell + '"' in hub], []),
        expect_false("the hub names no rules_elixir target", "@rules_elixir" in hub),
        expect_contains("the hub names the rules_elixir type", hub, "//:toolchain_type"),
        expect_false("launcher keeps no bare options", "-ebin $ELIXIR_ERL_OPTIONS" in script),
        expect_equal("only the elixir windows launcher is kept", [path for path in DROPPED_PATHS if path.endswith(".bat") or path.endswith(".ps1")], ["bin/iex.bat", "bin/mix.bat", "bin/mix.ps1"]),
        expect_contains("windows launcher forces Unicode names and stdio", elixir_bat(_BAT_SOURCE), '"%ERTS_BIN%erl.exe" +fnu -kernel standard_io_encoding unicode %ELIXIR_ERL_OPTIONS%'),
        expect_contains("launcher converts the script path on msys", script, '[ "$OSTYPE" = "msys" ]; then SCRIPT_PATH'),
    ] + _credo_checks() + _launcher_checks()

def elixir_metadata_tests(name, size):
    """Declares the Elixir toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
