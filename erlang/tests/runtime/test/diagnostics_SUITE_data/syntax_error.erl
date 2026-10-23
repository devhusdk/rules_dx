-module(syntax_error).

-export([add/2]).

add(A, B) -> A + .
