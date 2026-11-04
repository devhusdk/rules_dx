-module(kv_store).

-export([run/1, watch/0, snapshot_json/0]).

-spec run(kv_proto:command()) -> ok | {ok, integer()} | {error, term()}.
run(Command) ->
    gen_server:call(kv_store_server, Command).

-spec watch() -> ok.
watch() ->
    gen_server:call(kv_store_server, {watch, self()}).

-spec snapshot_json() -> binary().
snapshot_json() ->
    iolist_to_binary(thoas:encode(gen_server:call(kv_store_server, snapshot))).
