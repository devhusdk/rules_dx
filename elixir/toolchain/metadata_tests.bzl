"""Elixir toolchain pin and launcher tests."""

load("//erlang/toolchain:pins.bzl", "CELLS", "OTP_VERSION")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "starlark_test")
load(":pins.bzl", "ELIXIR_APPLICATIONS", "ELIXIR_CELLS", "ELIXIR_OTP_MAJOR", "ELIXIR_SHA256", "ELIXIR_URL", "ELIXIR_VERSION", "RULES_ELIXIR_COMMIT", "RULES_ELIXIR_SHA256", "RULES_ELIXIR_URL")
load(":repos.bzl", "DROPPED_PATHS", "elixir_script", "hub_build")

_SOURCE = """#!/bin/sh
set -- "$ERTS_BIN$ERL_EXEC" -noshell -pa "$SCRIPT_PATH"/../lib/elixir/ebin $ELIXIR_ERL_OPTIONS $ERL "$@"
"""

def _checks():
    script = elixir_script(_SOURCE)
    hub = hub_build(ELIXIR_CELLS)
    return [
        expect_equal("elixir version", ELIXIR_VERSION, "1.20.4"),
        expect_equal("build matches the OTP major", ELIXIR_OTP_MAJOR, OTP_VERSION.split(".")[0]),
        expect_equal("distribution url", ELIXIR_URL, "https://builds.hex.pm/builds/elixir/v1.20.4-otp-29.zip"),
        expect_equal("distribution digest", len(ELIXIR_SHA256), 64),
        expect_equal("applications", ELIXIR_APPLICATIONS, ["eex", "elixir", "ex_unit", "iex", "logger", "mix"]),
        expect_equal("rules_elixir commit", len(RULES_ELIXIR_COMMIT), 40),
        expect_equal("rules_elixir digest", len(RULES_ELIXIR_SHA256), 64),
        expect_contains("rules_elixir url names the commit", RULES_ELIXIR_URL, RULES_ELIXIR_COMMIT),
        expect_contains("launcher forces Unicode names and stdio", script, "+fnu -kernel standard_io_encoding unicode $ELIXIR_ERL_OPTIONS $ERL"),
        expect_equal("elixir runs only on qualified cells", ELIXIR_CELLS, ["linux_amd64"]),
        expect_equal("the hub registers each Elixir cell", hub.count("toolchain("), len(ELIXIR_CELLS)),
        expect_equal("the hub skips other OTP cells", [cell for cell in CELLS if cell not in ELIXIR_CELLS and '"' + cell + '"' in hub], []),
        expect_false("launcher keeps no bare options", "-ebin $ELIXIR_ERL_OPTIONS" in script),
        expect_equal("windows launchers are dropped", [path for path in DROPPED_PATHS if path.endswith(".bat") or path.endswith(".ps1")], ["bin/elixir.bat", "bin/elixirc.bat", "bin/iex.bat", "bin/mix.bat", "bin/mix.ps1"]),
    ]

def elixir_metadata_tests(name):
    """Declares the Elixir toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
