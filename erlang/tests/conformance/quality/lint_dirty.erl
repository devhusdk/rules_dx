-module(lint_dirty).

-export([show/1]).

show(X) -> io:format("~p~n", [X]).
