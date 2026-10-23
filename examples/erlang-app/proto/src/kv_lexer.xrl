Definitions.

WORD = [a-z][a-z0-9_]*
INT = [0-9]+
WS = [\s\t]+

Rules.

{WORD} : word(TokenChars, TokenLine).
{INT} : {token, {int, TokenLine, list_to_integer(TokenChars)}}.
{WS} : skip_token.

Erlang code.

word("put", Line) -> {token, {put, Line}};
word("get", Line) -> {token, {get, Line}};
word("del", Line) -> {token, {del, Line}};
word(Chars, Line) -> {token, {key, Line, list_to_binary(Chars)}}.
