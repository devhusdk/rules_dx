-module(hello).

-export([main/1]).

main(_Args) ->
    io:format("~s~n", [greet:greet("world")]).
