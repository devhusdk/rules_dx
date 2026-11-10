"""Public Erlang API."""

load(
    "//erlang/private:defs.bzl",
    _ERLANG_CELLS = "ERLANG_CELLS",
    _ERLANG_COMPATIBLE_WITH = "ERLANG_COMPATIBLE_WITH",
    _ErlangAppInfo = "ErlangAppInfo",
    _ERLANG_NIF_COMPATIBLE_WITH = "ERLANG_NIF_COMPATIBLE_WITH",
    _ERLANG_DIALYZER_COMPATIBLE_WITH = "ERLANG_DIALYZER_COMPATIBLE_WITH",
    _DEFAULT_TEST_ERLC_OPTS = "DEFAULT_TEST_ERLC_OPTS",
    _erlc_opts = "erlc_opts",
    _erlang_xrl_yrl = "erlang_xrl_yrl",
    _ERLANG_SOURCE_GLOBS = "ERLANG_SOURCE_GLOBS",
    _erlang_nif = "erlang_nif",
    _erlang_release = "erlang_release",
    _erlang_app = "erlang_app",
    _erlang_test_app = "erlang_test_app",
    _erlang_bytecode = "erlang_bytecode",
    _erlang_escript_archive = "erlang_escript_archive",
    _erlang_eunit_test = "erlang_eunit_test",
    _erlang_ct_suite_test = "erlang_ct_suite_test",
    _erlang_bench = "erlang_bench",
)

ERLANG_CELLS = _ERLANG_CELLS
ERLANG_COMPATIBLE_WITH = _ERLANG_COMPATIBLE_WITH
ErlangAppInfo = _ErlangAppInfo
ERLANG_NIF_COMPATIBLE_WITH = _ERLANG_NIF_COMPATIBLE_WITH
ERLANG_DIALYZER_COMPATIBLE_WITH = _ERLANG_DIALYZER_COMPATIBLE_WITH
DEFAULT_TEST_ERLC_OPTS = _DEFAULT_TEST_ERLC_OPTS
erlc_opts = _erlc_opts
erlang_xrl_yrl = _erlang_xrl_yrl
ERLANG_SOURCE_GLOBS = _ERLANG_SOURCE_GLOBS
erlang_nif = _erlang_nif
erlang_release = _erlang_release
erlang_app = _erlang_app
erlang_test_app = _erlang_test_app
erlang_bytecode = _erlang_bytecode
erlang_escript_archive = _erlang_escript_archive
erlang_eunit_test = _erlang_eunit_test
erlang_ct_suite_test = _erlang_ct_suite_test
erlang_bench = _erlang_bench
