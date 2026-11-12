-module(kv_store_SUITE).

-include_lib("common_test/include/ct.hrl").
-include_lib("stdlib/include/assert.hrl").

-export([all/0, init_per_testcase/2, end_per_testcase/2]).
-export([stores_values/1, enforces_configured_limit/1, notifies_watchers/1,
         restarts_after_crash/1, exports_json_snapshot/1]).

all() ->
    [stores_values, enforces_configured_limit, notifies_watchers,
     restarts_after_crash, exports_json_snapshot].

init_per_testcase(_Case, Config) ->
    {ok, _} = application:ensure_all_started(kv_store),
    Config.

end_per_testcase(_Case, _Config) ->
    ok = application:stop(kv_store).

stores_values(_Config) ->
    ok = kv_store:run({put, <<"apples">>, 3}),
    ?assertEqual({ok, 3}, kv_store:run({get, <<"apples">>})),
    ok = kv_store:run({del, <<"apples">>}),
    ?assertEqual({error, not_found}, kv_store:run({get, <<"apples">>})).

enforces_configured_limit(_Config) ->
    ?assertEqual({ok, 3}, application:get_env(kv_store, max_keys)),
    [ok = kv_store:run({put, Key, 1}) || Key <- [<<"a">>, <<"b">>, <<"c">>]],
    ?assertEqual({error, full}, kv_store:run({put, <<"d">>, 1})),
    ?assertEqual(ok, kv_store:run({put, <<"a">>, 2})).

notifies_watchers(_Config) ->
    ok = kv_store:watch(),
    ok = kv_store:run({put, <<"pears">>, 5}),
    receive
        {kv_event, Event} -> ?assertEqual({put, <<"pears">>, 5}, Event)
    after 5000 ->
        ct:fail(no_event)
    end.

restarts_after_crash(_Config) ->
    Before = whereis(kv_store_server),
    Ref = monitor(process, Before),
    exit(Before, kill),
    receive
        {'DOWN', Ref, process, Before, killed} -> ok
    after 5000 ->
        ct:fail(not_killed)
    end,
    After = wait_for_restart(Before, 50),
    ?assertNotEqual(Before, After),
    ?assertEqual({error, not_found}, kv_store:run({get, <<"pears">>})).

exports_json_snapshot(_Config) ->
    ok = kv_store:run({put, <<"plums">>, 4}),
    ?assertEqual(<<"{\"plums\":4}">>, kv_store:snapshot_json()).

wait_for_restart(_Old, 0) ->
    ct:fail(not_restarted);
wait_for_restart(Old, Tries) ->
    case whereis(kv_store_server) of
        Pid when is_pid(Pid), Pid =/= Old -> Pid;
        _ ->
            timer:sleep(20),
            wait_for_restart(Old, Tries - 1)
    end.
