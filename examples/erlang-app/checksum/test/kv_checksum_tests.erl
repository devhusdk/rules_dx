-module(kv_checksum_tests).

-include_lib("eunit/include/eunit.hrl").

empty_binary_is_the_offset_basis_test() ->
    ?assertEqual(16#811c9dc5, kv_checksum:fnv1a(<<>>)).

known_vectors_test() ->
    ?assertEqual(16#e40c292c, kv_checksum:fnv1a(<<"a">>)),
    ?assertEqual(16#bf9cf968, kv_checksum:fnv1a(<<"foobar">>)).

non_binary_is_badarg_test() ->
    ?assertError(badarg, kv_checksum:fnv1a("foobar")).
