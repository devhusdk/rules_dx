-module(sum).

-export([fold/1, list/1]).

fold(List) -> lists:foldl(fun(X, Acc) -> X + Acc end, 0, List).

list(List) -> lists:sum(List).
