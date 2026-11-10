#!/usr/bin/env escript
%% Usage: plt_build.escript DIALYZER OUT DIALYZER_ARGS...
%% Runs dialyzer with --output_plt OUT. Dialyzer exits 2 when it only emitted warnings, which a PLT
%% build does for unresolved calls; only other non-zero exits fail, and then its output is printed.
%% Then makes the PLT independent of where the OTP tree was when it was built.
%%
%% A classic PLT (record `file_plt`, version v6.x) stores the absolute path of every
%% analysed beam, and of dialyzer's own erl_types/erl_bif_types beams. Dialyzer reads the
%% latter on every start, even with --no_check_plt, so a PLT built in one sandbox fails in
%% another with "File not found". Beams of another language (Elixir) sit under the action's working
%% directory and move to /dx_execroot, so the PLT is the same in every sandbox. The PLT is rebuilt
%% whenever OTP changes, so the record layout is checked here and a change fails the build instead of corrupting the PLT.
main([Dialyzer, Out | Args]) ->
    ensure_home(),
    Port = open_port({spawn_executable, filename:absname(Dialyzer)},
                     [{args, Args ++ ["--output_plt", Out]}, exit_status, binary, stderr_to_stdout, hide]),
    case collect(Port, []) of
        {Status, _Log} when Status =:= 0; Status =:= 2 ->
            fix(Out);
        {_Status, Log} ->
            io:format(standard_error, "~s", [Log]),
            halt(1)
    end.

%% Dialyzer reads the user's home for its caches, and a hermetic action has none.
ensure_home() ->
    case os:getenv("HOME") of
        false ->
            Home = case os:getenv("USERPROFILE") of
                       false -> os:getenv("TMPDIR", "/tmp");
                       Profile -> Profile
                   end,
            true = os:putenv("HOME", Home);
        _ -> ok
    end.

collect(Port, Acc) ->
    receive
        {Port, {data, Data}} -> collect(Port, [Acc, Data]);
        {Port, {exit_status, Status}} -> {Status, Acc}
    end.

fix(Out) ->
    {ok, Bin} = file:read_file(Out),
    Root = code:root_dir(),
    {ok, Cwd} = file:get_cwd(),
    Plt = binary_to_term(Bin),
    case {element(1, Plt), tuple_size(Plt)} of
        {file_plt, 10} ->
            Files = [{relocate(File, Root, Cwd), Md5} || {File, Md5} <- element(3, Plt)],
            Fixed = setelement(10, setelement(3, Plt, Files), []),
            ok = file:write_file(Out, term_to_binary(Fixed, [compressed]));
        Other ->
            io:format(standard_error, "unsupported PLT layout: ~p~n", [Other]),
            halt(1)
    end.

relocate(File, Root, Cwd) ->
    case string:prefix(File, Root) of
        nomatch ->
            case string:prefix(File, Cwd) of
                nomatch -> File;
                Rest -> "/dx_execroot" ++ Rest
            end;
        Rest ->
            "/dx_otp" ++ Rest
    end.
