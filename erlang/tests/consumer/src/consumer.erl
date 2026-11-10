-module(consumer).

-export([greet/1]).

-spec greet(binary()) -> binary().
greet(Name) ->
    <<"hello, ", Name/binary>>.
