-module(export_all_SUITE).

-compile(export_all).

-include_lib("stdlib/include/assert.hrl").

all() ->
    [exports_every_function].

exports_every_function(_Config) ->
    ?assert(erlang:function_exported(?MODULE, exports_every_function, 1)).
