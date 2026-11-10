#!/usr/bin/env escript
%% Reads Erlang sources with the pinned OTP's edoc and writes their public API as versioned JSON.
%%
%% usage: edoc_dump.escript SRCS OUT
%% SRCS lists one source per line as `exec path TAB repo-relative path TAB 0|1 (generated)`.

-include_lib("xmerl/include/xmerl.hrl").

main([Srcs, Out]) ->
    Rows = rows(Srcs),
    Symbols = lists:append([file_symbols(Row) || Row <- Rows]),
    Json = json(#{<<"format">> => 1, <<"language">> => <<"erlang">>, <<"tool">> => <<"edoc">>,
                  <<"version">> => list_to_binary(erlang:system_info(otp_release)),
                  <<"symbols">> => Symbols}),
    ok = file:write_file(Out, unicode:characters_to_binary(Json));
main(_) ->
    fail("usage: edoc_dump.escript SRCS OUT").

fail(Message) ->
    io:format(standard_error, "edoc_dump: ~s~n", [Message]),
    halt(1).

rows(Path) ->
    {ok, Bin} = file:read_file(Path),
    [case binary:split(Line, <<"\t">>, [global]) of
         [Exec, Rel, Gen] -> {binary_to_list(Exec), Rel, Gen =:= <<"1">>};
         _ -> fail("malformed manifest line")
     end || Line <- binary:split(Bin, <<"\n">>, [global, trim_all])].

file_symbols({Exec, Rel, Generated}) ->
    {Module, Doc} = try edoc:get_doc(Exec, [{preprocess, false}])
                    catch Class:Reason -> fail(io_lib:format("edoc failed on ~s: ~p:~p", [Exec, Class, Reason]))
                    end,
    Name = atom_to_binary(Module, utf8),
    Lines = function_lines(Exec),
    Base = #{<<"file">> => Rel, <<"generated">> => Generated},
    Mod = maps:merge(Base, #{<<"kind">> => <<"module">>, <<"module">> => Name, <<"name">> => <<>>,
                             <<"arity">> => 0, <<"signature">> => <<"-module(", Name/binary, ").">>,
                             <<"params">> => [], <<"doc">> => description(Doc),
                             <<"line">> => 1}),
    Funs = [function(F, Name, Base, Lines) || F <- children(Doc, functions, function),
            attr(F, exported) =:= "yes"],
    [Mod | Funs].

function(F, Module, Base, Lines) ->
    Fun = list_to_binary(attr(F, name)),
    Arity = list_to_integer(attr(F, arity)),
    Args = [list_to_binary(text(A)) || A <- descend(F, [args, arg, argName])],
    Signature = iolist_to_binary([Fun, "(", lists:join(", ", Args), ")"]),
    maps:merge(Base, #{<<"kind">> => <<"function">>, <<"module">> => Module, <<"name">> => Fun,
                       <<"arity">> => Arity, <<"signature">> => Signature, <<"params">> => Args,
                       <<"doc">> => description(F),
                       <<"line">> => maps:get({binary_to_atom(Fun, utf8), Arity}, Lines, 1)}).

function_lines(Exec) ->
    {ok, Forms} = epp_dodger:parse_file(Exec),
    maps:from_list([{{erl_syntax:atom_value(erl_syntax:function_name(F)), erl_syntax:function_arity(F)},
                     erl_anno:line(erl_syntax:get_pos(F))}
                    || F <- Forms, erl_syntax:type(F) =:= function]).

%% xmerl helpers
children(Element, Group, Name) ->
    lists:append([[C || C <- content(G), is_element(C, Name)] || G <- content(Element), is_element(G, Group)]).

descend(Element, []) -> [Element];
descend(Element, [Name | Rest]) ->
    lists:append([descend(C, Rest) || C <- content(Element), is_element(C, Name)]).

is_element(#xmlElement{name = N}, N) -> true;
is_element(_, _) -> false.

content(#xmlElement{content = C}) -> C;
content(_) -> [].

attr(#xmlElement{attributes = As}, Name) ->
    case lists:keyfind(Name, #xmlAttribute.name, As) of
        #xmlAttribute{value = V} -> V;
        false -> ""
    end.

description(Element) ->
    Full = [text(D) || D <- descend(Element, [description, fullDescription])],
    Brief = [text(D) || D <- descend(Element, [description, briefDescription])],
    list_to_binary(string:trim(case Full of [T | _] -> T; [] -> case Brief of [B | _] -> B; [] -> "" end end)).

text(#xmlText{value = V}) -> V;
text(#xmlElement{name = code, content = C}) -> "`" ++ lists:append([text(X) || X <- C]) ++ "`";
text(#xmlElement{name = p, content = C}) -> "\n\n" ++ lists:append([text(X) || X <- C]);
text(#xmlElement{content = C}) -> lists:append([text(X) || X <- C]);
text(_) -> "".

%% A minimal JSON writer: no JSON library ships with every pinned OTP cell.
json(Map) when is_map(Map) ->
    Pairs = [[json(K), ":", json(V)] || {K, V} <- lists:sort(maps:to_list(Map))],
    ["{", lists:join(",", Pairs), "}"];
json(List) when is_list(List) -> ["[", lists:join(",", [json(E) || E <- List]), "]"];
json(true) -> "true";
json(false) -> "false";
json(N) when is_integer(N) -> integer_to_list(N);
json(B) when is_binary(B) -> [$", escape(unicode:characters_to_list(B)), $"].

escape(Chars) -> lists:append([esc(C) || C <- Chars]).
esc($") -> "\\\"";
esc($\\) -> "\\\\";
esc($\n) -> "\\n";
esc($\r) -> "\\r";
esc($\t) -> "\\t";
esc(C) when C < 32 -> io_lib:format("\\u~4.16.0b", [C]);
esc(C) -> [C].
