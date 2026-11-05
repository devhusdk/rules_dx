-module(kv_checksum).

-export([fnv1a/1]).
-nifs([fnv1a/1]).
-on_load(init/0).

-spec fnv1a(binary()) -> non_neg_integer().
fnv1a(_Data) ->
    erlang:nif_error(not_loaded). % LCOV_EXCL_LINE reason: replaced by the NIF at load, issue: 1789

init() ->
    Priv = filename:join([filename:dirname(code:which(?MODULE)), "..", "priv", "kv_checksum_nif"]),
    erlang:load_nif(Priv, 0).
