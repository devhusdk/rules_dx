#!/usr/bin/env escript
%% Replays one erlang_bytecode compilation with erlc; writes the output log and the exit code.

main(Argv) ->
    {Flags, Srcs} = split(Argv, []),
    Get = fun(Key) -> proplists:get_value(Key, Flags) end,
    All = fun(Key) -> [V || {K, V} <- Flags, K =:= Key] end,
    Bin = filename:join(code:root_dir(), "bin"),
    Exe = case os:type() of
        {win32, _} -> ".exe";
        _ -> ""
    end,
    Erlc = filename:join(Bin, "erlc" ++ Exe),
    Out = Get("out"),
    Env = case Get("erl-libs") of
        undefined -> [];
        Libs -> [{"ERL_LIBS", Libs}]
    end,
    Base = ["-v" | All("erlc-arg")],
    Opts = All("opt"),
    Erlc1 = fun(Extra) -> run(Erlc, Env, Base ++ Extra) end,
    First = case Get("compile-first") of
        undefined -> [];
        Script ->
            {_, Listed} = run(filename:join(Bin, "escript" ++ Exe), Env, [Script | Srcs]),
            string:tokens(binary_to_list(Listed), " \t\r\n")
    end,
    {Code, Log} = case First of
        [] ->
            Erlc1(["-o", Out | Opts] ++ Srcs);
        _ ->
            {Code1, Log1} = Erlc1(["-o", Out | Opts] ++ First),
            case Code1 of
                0 ->
                    {Code2, Log2} = Erlc1(["-pa", Out, "-o", Out | Opts] ++ Srcs),
                    {Code2, <<Log1/binary, Log2/binary>>};
                _ ->
                    {Code1, Log1}
            end
    end,
    ok = file:write_file(Get("log"), Log),
    ok = file:write_file(Get("status"), integer_to_list(Code) ++ "\n").

split(["--" | Srcs], Acc) ->
    {lists:reverse(Acc), Srcs};
split(["--" ++ Flag | Rest], Acc) ->
    [Key, Value] = string:split(Flag, "=", leading),
    split(Rest, [{Key, Value} | Acc]).

run(Exe, Env, Args) ->
    Port = open_port({spawn_executable, Exe}, [
        {args, Args}, {env, Env}, binary, exit_status, hide, stderr_to_stdout
    ]),
    collect(Port, <<>>).

collect(Port, Acc) ->
    receive
        {Port, {data, Data}} -> collect(Port, <<Acc/binary, Data/binary>>);
        {Port, {exit_status, Status}} -> {Status, Acc}
    end.
