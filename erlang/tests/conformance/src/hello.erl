-module(hello).

-export([main/1]).

-spec main([string()]) -> ok.
main([Path]) ->
    {ok, Text} = file:read_file(rlocation(Path)),
    io:put_chars(Text);
main(_Args) ->
    io:format("~s~n", [greet:greet("world")]).

-spec rlocation(string()) -> string().
rlocation(Path) ->
    case os:getenv("RUNFILES_DIR") of
        false -> manifest_lookup(os:getenv("RUNFILES_MANIFEST_FILE"), Path);
        Dir -> filename:join(Dir, Path)
    end.

-spec manifest_lookup(string(), string()) -> string().
manifest_lookup(Manifest, Path) ->
    {ok, Text} = file:read_file(Manifest),
    Lines = string:split(unicode:characters_to_list(Text), "\n", all),
    Key = Path ++ " ",
    [Target | _] = [string:trim(string:prefix(Line, Key), trailing, "\r") || Line <- Lines, string:prefix(Line, Key) =/= nomatch],
    Target.
