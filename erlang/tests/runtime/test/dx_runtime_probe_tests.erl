-module(dx_runtime_probe_tests).

-include_lib("eunit/include/eunit.hrl").

reads_the_pinned_version_test() ->
    ?assertEqual(os:getenv("DX_OTP_VERSION"), dx_runtime_probe:otp_version()).

runs_from_the_pinned_tree_test() ->
    ?assertNotEqual(nomatch, string:find(code:root_dir(), "dx_otp_")).
