-module(kvctl).

-export([main/1, run/1]).

main(Args) ->
    {ok, _} = application:ensure_all_started(kv_store),
    {Lines, Code} = commands(Args),
    [io:format("~s~n", [Line]) || Line <- Lines],
    halt(Code).

-spec run([string()]) -> {[string()], 0 | 1}.
run(Args) ->
    {ok, _} = application:ensure_all_started(kv_store),
    Result = commands(Args),
    ok = application:stop(kv_store),
    Result.

commands(Args) ->
    {Lines, Code} = lists:foldl(fun step/2, {[], 0}, Args),
    Snapshot = binary_to_list(kv_store:snapshot_json()),
    {lists:reverse([Snapshot | Lines]), Code}.

step(Arg, {Lines, Code}) ->
    case kv_proto:parse(Arg) of
        {ok, Command} ->
            {[kv_proto:format(kv_store:run(Command)) | Lines], Code};
        Error ->
            {[kv_proto:format(Error) | Lines], 1}
    end.
