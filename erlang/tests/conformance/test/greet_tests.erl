-module(greet_tests).

-include_lib("eunit/include/eunit.hrl").

passing_case_test() ->
    ?assertEqual("hello world", greet:greet("world")).
