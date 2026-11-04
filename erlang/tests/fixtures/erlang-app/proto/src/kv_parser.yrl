Nonterminals command.
Terminals put get del key int.
Rootsymbol command.

command -> put key int : {put, value('$2'), value('$3')}.
command -> get key : {get, value('$2')}.
command -> del key : {del, value('$2')}.

Erlang code.

value({_, _, Value}) -> Value.
