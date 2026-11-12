-module(kv_store_sup).
-behaviour(supervisor).

-export([start_link/0, init/1]).

start_link() ->
    supervisor:start_link({local, ?MODULE}, ?MODULE, []).

init([]) ->
    Flags = #{strategy => one_for_one, intensity => 5, period => 10},
    Server = #{id => kv_store_server, start => {kv_store_server, start_link, []}},
    {ok, {Flags, [Server]}}.
