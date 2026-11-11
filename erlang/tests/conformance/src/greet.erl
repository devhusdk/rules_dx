-module(greet).

-export([greet/1]).

-spec greet(string()) -> string().
greet(Name) ->
    "hello " ++ Name.
