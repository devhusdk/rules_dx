-module(kv_tests).
-include_lib("eunit/include/eunit.hrl").

get_test() -> ?assertEqual(a, kv:get(a)).
