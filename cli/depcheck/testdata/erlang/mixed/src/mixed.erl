-module(mixed).
-include_lib("cowlib/include/cow_inets.hrl").
-export([start/0]).

start() ->
    ranch:start_listener(mixed, ranch_tcp, #{}, mixed_protocol, []),
    jsx:encode(#{}).
