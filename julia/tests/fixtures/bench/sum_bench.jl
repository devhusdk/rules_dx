using DxBench

numbers = collect(1:1000)

DxBench.run_benchmarks([
    "sum_loop" => () -> sum(numbers),
    "sum_join" => () -> length(join(numbers, "+")),
])
