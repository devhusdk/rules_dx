"""Public Erlang API."""

load(
    "//erlang/private:defs.bzl",
    _ErlangAppInfo = "ErlangAppInfo",
    _erlc_opts = "erlang_erlc_opts",
    _erlang_xrl_yrl = "erlang_xrl_yrl",
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

ErlangAppInfo = _ErlangAppInfo
erlang_erlc_opts = _erlc_opts
erlang_xrl_yrl = _erlang_xrl_yrl
erlang_nif = _erlang_nif
erlang_release = _erlang_release
erlang_app = _erlang_app
erlang_test_app = _erlang_test_app
erlang_bytecode = _erlang_bytecode
erlang_escript_archive = _erlang_escript_archive
erlang_eunit_test = _erlang_eunit_test
erlang_ct_suite_test = _erlang_ct_suite_test
erlang_bench = _erlang_bench
