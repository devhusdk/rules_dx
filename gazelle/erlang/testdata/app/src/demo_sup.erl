-module(demo_sup).
-export([start_link/0]).

start_link() -> lists:map(fun demo_app:stop/1, []), ok.
