-module(dx_cpp_nif_tests).

-include_lib("eunit/include/eunit.hrl").

exceptions_are_caught_inside_each_nif_test() ->
    ?assertMatch({1, 64, _}, dx_cpp_a:probe()),
    ?assertMatch({1, 64, _}, dx_cpp_b:probe()).

statics_are_private_to_each_nif_test() ->
    {1, 64, A1} = dx_cpp_a:probe(),
    {1, 64, A2} = dx_cpp_a:probe(),
    ?assertEqual(A1 + 1, A2),
    {1, 64, B1} = dx_cpp_b:probe(),
    ?assert(B1 >= 1).
