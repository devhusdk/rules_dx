-module(dx_erlc_unused).
-export([run/1]).

run(Value) ->
    Unused = Value + 1,
    ok.

helper() ->
    ok.
