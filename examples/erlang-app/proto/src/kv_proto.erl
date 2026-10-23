-module(kv_proto).

-export([parse/1, format/1]).

-type command() :: {put, binary(), integer()} | {get, binary()} | {del, binary()}.
-export_type([command/0]).

-spec parse(string()) -> {ok, command()} | {error, term()}.
parse(Line) ->
    case kv_lexer:string(Line) of
        {ok, [], _} ->
            {error, empty};
        {ok, Tokens, _} ->
            case kv_parser:parse(Tokens) of
                {ok, Command} -> {ok, Command};
                {error, {_, kv_parser, Message}} -> {error, {syntax, lists:flatten(kv_parser:format_error(Message))}}
            end;
        {error, {_, kv_lexer, Reason}, _} ->
            {error, {lexical, lists:flatten(kv_lexer:format_error(Reason))}}
    end.

-spec format({ok, integer()} | ok | {error, term()}) -> string().
format(ok) -> "ok";
format({ok, Value}) -> integer_to_list(Value);
format({error, Reason}) -> lists:flatten(io_lib:format("error: ~p", [Reason])).
