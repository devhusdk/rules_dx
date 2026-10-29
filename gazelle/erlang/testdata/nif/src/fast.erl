-module(fast).
-export([init/0, add/2]).
-on_load(init/0).

init() -> erlang:load_nif("./priv/fast_nif", 0).

add(_A, _B) -> erlang:nif_error(not_loaded).
