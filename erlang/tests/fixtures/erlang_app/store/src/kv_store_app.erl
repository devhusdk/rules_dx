-module(kv_store_app).
-behaviour(application).

-export([start/2, stop/1]).

start(_Type, _Args) ->
    kv_store_sup:start_link().

stop(_State) ->
    ok.
