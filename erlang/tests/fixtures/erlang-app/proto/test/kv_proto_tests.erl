-module(kv_proto_tests).

-include_lib("eunit/include/eunit.hrl").

put_test() ->
    ?assertEqual({ok, {put, <<"apples">>, 3}}, kv_proto:parse("put apples 3")).

get_and_del_test() ->
    ?assertEqual({ok, {get, <<"pears">>}}, kv_proto:parse("get pears")),
    ?assertEqual({ok, {del, <<"pears">>}}, kv_proto:parse("  del\tpears ")).

empty_line_is_rejected_test() ->
    ?assertEqual({error, empty}, kv_proto:parse("   ")).

missing_value_is_a_syntax_error_test() ->
    ?assertMatch({error, {syntax, _}}, kv_proto:parse("put apples")).

unknown_character_is_a_lexical_error_test() ->
    ?assertMatch({error, {lexical, _}}, kv_proto:parse("put Apples 3")).

format_test() ->
    ?assertEqual("ok", kv_proto:format(ok)),
    ?assertEqual("7", kv_proto:format({ok, 7})),
    ?assertEqual("error: not_found", kv_proto:format({error, not_found})).
