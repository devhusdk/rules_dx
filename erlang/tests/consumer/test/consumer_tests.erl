-module(consumer_tests).

-include_lib("eunit/include/eunit.hrl").

greet_test() ->
    ?assertEqual(<<"hello, dx">>, consumer:greet(<<"dx">>)).
