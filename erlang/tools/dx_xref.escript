#!/usr/bin/env escript
%% Usage: dx_xref.escript [--pa DIR]... [--dep BEAM]... OWNED_BEAM...
%% Lists the exported functions of the owned beams that no analysed module calls, one
%% `file:line: Module:Function/Arity` per line. Dependency beams are analysed for their calls only.
%% Behaviour callbacks, module_info and generated `__` functions (Elixir) are not reported.
main(Args) ->
    {Pa, Deps, Owned} = split(Args, [], [], []),
    lists:foreach(fun(Dir) -> true = code:add_pathz(Dir) end, Pa),
    {ok, _} = xref:start(dx_xref, [{xref_mode, modules}]),
    ok = xref:set_default(dx_xref, [{verbose, false}, {warnings, false}, {builtins, false}]),
    Seen = lists:foldl(fun(Beam, Acc) -> add(Beam, Acc) end, #{}, Owned ++ Deps),
    OwnedMods = [module(Beam) || Beam <- Owned],
    {ok, Unused} = xref:analyze(dx_xref, exports_not_used),
    Forms = maps:from_list([{M, forms(maps:get(M, Seen))} || M <- OwnedMods, maps:is_key(M, Seen)]),
    Report = lists:sort([line(MFA, Forms) || {M, _, _} = MFA <- Unused, maps:is_key(M, Forms),
                         not skipped(MFA, maps:get(M, Forms))]),
    lists:foreach(fun(Line) -> io:format("~ts~n", [Line]) end, Report),
    xref:stop(dx_xref).

split(["--pa", Dir | Rest], Pa, Deps, Owned) -> split(Rest, [Dir | Pa], Deps, Owned);
split(["--dep", Beam | Rest], Pa, Deps, Owned) -> split(Rest, Pa, [Beam | Deps], Owned);
split([Beam | Rest], Pa, Deps, Owned) -> split(Rest, Pa, Deps, [Beam | Owned]);
split([], Pa, Deps, Owned) -> {lists:reverse(Pa), lists:reverse(Deps), lists:reverse(Owned)}.

module(Beam) -> list_to_atom(filename:basename(Beam, ".beam")).

add(Beam, Seen) ->
    Mod = module(Beam),
    case maps:is_key(Mod, Seen) of
        true -> Seen;
        false ->
            true = code:add_pathz(filename:dirname(Beam)),
            {ok, Mod} = xref:add_module(dx_xref, Beam),
            Seen#{Mod => Beam}
    end.

%% The abstract forms of a beam, from its debug info, whichever compiler wrote it.
forms(Beam) ->
    case beam_lib:chunks(Beam, [debug_info]) of
        {ok, {Mod, [{debug_info, {debug_info_v1, Backend, Data}}]}} ->
            try Backend:debug_info(erlang_v1, Mod, Data, []) of
                {ok, Forms} -> Forms;
                _ -> []
            catch
                _:_ -> []
            end;
        _ -> []
    end.

skipped({M, F, A}, Forms) ->
    Name = atom_to_list(F),
    F =:= module_info orelse lists:prefix("__", Name) orelse lists:member({F, A}, callbacks(M, Forms)).

callbacks(_M, Forms) ->
    lists:append([behaviour_callbacks(B) || {attribute, _, Attr, B} <- Forms, Attr =:= behaviour orelse Attr =:= behavior]).

behaviour_callbacks(Behaviour) ->
    try Behaviour:behaviour_info(callbacks) of
        List when is_list(List) -> List;
        _ -> []
    catch
        _:_ -> []
    end.

line({M, F, A}, Forms) ->
    Fs = maps:get(M, Forms),
    File = case [N || {attribute, _, file, {N, _}} <- Fs] of
               [N | _] -> N;
               [] -> atom_to_list(M)
           end,
    Line = case [erl_anno:line(Anno) || {function, Anno, F1, A1, _} <- Fs, F1 =:= F, A1 =:= A] of
               [L | _] -> L;
               [] -> 1
           end,
    lists:flatten(io_lib:format("~ts:~b: ~ts:~ts/~b", [File, Line, M, F, A])).
