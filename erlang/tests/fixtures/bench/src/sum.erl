-module(sum).

-export([fold/1, list/1]).

-spec fold([number()]) -> number().
fold(List) -> lists:foldl(fun(X, Acc) -> X + Acc end, 0, List).

-spec list([number()]) -> number().
list(List) -> lists:sum(List).
