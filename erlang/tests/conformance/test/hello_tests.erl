-module(hello_tests).

-include_lib("eunit/include/eunit.hrl").

failing_case_test() ->
    ?assertEqual("goodbye world", greet:greet("world")).
