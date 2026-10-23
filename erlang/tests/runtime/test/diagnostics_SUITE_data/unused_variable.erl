-module(unused_variable).

-export([add/2]).

add(A, B) -> Unused = A, B.
