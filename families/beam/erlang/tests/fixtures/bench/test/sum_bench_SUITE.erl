-module(sum_bench_SUITE).

-export([all/0, sums/1]).

all() ->
    [sums].

sums(_Config) ->
    List = lists:seq(1, 1000),
    dx_bench:run(
        #{fold => fun() -> sum:fold(List) end, list => fun() -> sum:list(List) end},
        #{samples => 2, sample_duration => 100, warmup => 0}
    ).
