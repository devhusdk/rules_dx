-module(needs_dep).

-include_lib("base_dep/include/base_dep.hrl").

-export([value/0]).

value() -> ?BASE_VALUE.
