-module(fast).
-export([add/2, lex/1]).
-on_load(init/0).

init() ->
    erlang:load_nif("priv/fast", 0).

add(_A, _B) -> erlang:nif_error(not_loaded).

lex(S) -> fast_lexer:string(S).
