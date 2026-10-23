-module(diagnostics_SUITE).

-include_lib("common_test/include/ct.hrl").
-include_lib("stdlib/include/assert.hrl").

-export([all/0]).
-export([runs_the_pinned_runtime/1, compiles_a_clean_module/1, rejects_a_syntax_error/1,
         treats_warnings_as_errors/1]).

all() ->
    [runs_the_pinned_runtime, compiles_a_clean_module, rejects_a_syntax_error,
     treats_warnings_as_errors].

runs_the_pinned_runtime(_Config) ->
    ?assertEqual(os:getenv("DX_OTP_VERSION"), dx_runtime_probe:otp_version()).

compiles_a_clean_module(Config) ->
    ?assertEqual({0, ""}, erlc(Config, "clean.erl")),
    ?assert(filelib:is_regular(filename:join(?config(priv_dir, Config), "clean.beam"))).

rejects_a_syntax_error(Config) ->
    {Status, Output} = erlc(Config, "syntax_error.erl"),
    ?assertEqual(1, Status),
    ?assertMatch({match, _}, re:run(Output, "syntax_error\\.erl:5:18: syntax error before: '\\.'")).

treats_warnings_as_errors(Config) ->
    {Status, Output} = erlc(Config, "unused_variable.erl"),
    ?assertEqual(1, Status),
    ?assertMatch({match, _}, re:run(Output, "unused_variable\\.erl:5:14: variable 'Unused' is unused")).

erlc(Config, Name) ->
    Erlc = filename:join([code:root_dir(), "bin", "erlc"]),
    Source = filename:join(?config(data_dir, Config), Name),
    Args = ["-Werror", "-o", ?config(priv_dir, Config), Source],
    Port = open_port({spawn_executable, Erlc}, [{args, Args}, exit_status, stderr_to_stdout]),
    collect(Port, []).

collect(Port, Acc) ->
    receive
        {Port, {data, Data}} -> collect(Port, [Acc | Data]);
        {Port, {exit_status, Status}} -> {Status, lists:flatten(Acc)}
    after 60000 ->
        ct:fail(erlc_timeout)
    end.
