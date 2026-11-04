Definitions and Rules.
D = [0-9]
Rules.
{D}+ : {token, {int, TokenLine, list_to_integer(TokenChars)}}.
Erlang code.
