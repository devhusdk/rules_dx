-module(dx_bench).

-export([run/1, run/2]).

%% Runs each fun with erlperf and writes dx-bench.json to the test outputs directory.
-spec run(#{term() => fun(() -> term())}) -> [map()].
run(Jobs) ->
    run(Jobs, #{}).

-spec run(#{term() => fun(() -> term())}, erlperf:run_options()) -> [map()].
run(Jobs, Options) ->
    {ok, _} = application:ensure_all_started(erlperf),
    Defaults = #{report => full, samples => 3, sample_duration => 200, warmup => 1},
    Results = [result(Name, Fun, maps:merge(Defaults, Options)) || {Name, Fun} <- lists:sort(maps:to_list(Jobs))],
    write(Results),
    Results.

result(Name, Fun, #{sample_duration := Duration} = Options) ->
    [#{result := #{average := Average, stddev := Stddev}}] = erlperf:benchmark([#{runner => Fun}], Options, undefined),
    Scale = 1000 / Duration,
    #{name => unicode:characters_to_binary(io_lib:format("~tp", [Name])),
      unit => <<"ops/s">>,
      mean => Average * Scale,
      stddev => Stddev * Scale,
      iterations => maps:get(samples, Options)}.

write(Results) ->
    case os:getenv("TEST_UNDECLARED_OUTPUTS_DIR") of
        false ->
            ok;
        Dir ->
            Document = #{schema => <<"dx-bench/v1">>, results => Results},
            ok = file:write_file(filename:join(Dir, "dx-bench.json"), json:encode(Document))
    end.
