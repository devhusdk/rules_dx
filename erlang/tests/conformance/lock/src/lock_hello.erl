-module(lock_hello).

-export([main/1]).

-spec main([string()]) -> ok.
main(_Args) ->
    {ok, Text} = thoas:decode(<<"\"hello lock\"">>),
    io:format("~s~n", [Text]).
