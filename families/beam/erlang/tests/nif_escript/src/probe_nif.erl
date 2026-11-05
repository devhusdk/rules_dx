-module(probe_nif).
-export([probe/0, main/1]).
-on_load(init/0).

init() ->
    erlang:load_nif(filename:join(code:priv_dir(nif_escript), "probe_nif"), 0).

probe() ->
    erlang:nif_error(not_loaded).

main(_) ->
    probe().
