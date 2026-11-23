-module(dx_cpp_a).

-export([probe/0]).
-nifs([probe/0]).
-on_load(init/0).

-spec probe() -> {0 | 1, non_neg_integer(), pos_integer()}.
probe() ->
    erlang:nif_error(not_loaded). % LCOV_EXCL_LINE reason: replaced by the NIF at load, issue: 4574

init() ->
    Priv = filename:join([filename:dirname(code:which(?MODULE)), "..", "priv", "dx_cpp_a_nif"]),
    erlang:load_nif(Priv, 0).
