-module(demo_app).
-behaviour(application).
-include("demo.hrl").
-export([start/2, stop/1]).

start(_Type, _Args) ->
    crypto:strong_rand_bytes(4),
    demo_sup:start_link().

stop(_State) -> ok.
