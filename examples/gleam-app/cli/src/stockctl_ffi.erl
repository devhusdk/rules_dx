-module(stockctl_ffi).
-export([arguments/0]).

arguments() ->
    [unicode:characters_to_binary(Argument) || Argument <- init:get_plain_arguments()].
