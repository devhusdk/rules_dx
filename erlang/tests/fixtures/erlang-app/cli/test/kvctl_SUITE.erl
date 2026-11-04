-module(kvctl_SUITE).

-include_lib("common_test/include/ct.hrl").
-include_lib("stdlib/include/assert.hrl").

-export([all/0]).
-export([runs_commands_in_process/1, escript_runs_on_the_pinned_runtime/1,
         escript_rejects_malformed_input/1]).

all() ->
    [runs_commands_in_process, escript_runs_on_the_pinned_runtime,
     escript_rejects_malformed_input].

runs_commands_in_process(_Config) ->
    ?assertEqual({["ok", "3", "{\"apples\":3}"], 0}, kvctl:run(["put apples 3", "get apples"])).

escript_runs_on_the_pinned_runtime(_Config) ->
    {Status, Output} = escript(["put plums 4", "get plums", "get figs"]),
    ?assertEqual(0, Status),
    ?assertEqual("ok\n4\nerror: not_found\n{\"plums\":4}\n", Output).

escript_rejects_malformed_input(_Config) ->
    {Status, Output} = escript(["put plums"]),
    ?assertEqual(1, Status),
    ?assertMatch("error: {syntax," ++ _, Output).

escript(Args) ->
    Port = open_port({spawn_executable, os:getenv("KVCTL")},
                     [{args, Args}, exit_status, stderr_to_stdout, binary]),
    collect(Port, <<>>).

collect(Port, Acc) ->
    receive
        {Port, {data, Data}} -> collect(Port, <<Acc/binary, Data/binary>>);
        {Port, {exit_status, Status}} -> {Status, binary_to_list(Acc)}
    after 60000 ->
        ct:fail(escript_timeout)
    end.
