using DxBench
using Test

@testset "report" begin
    dir = mktempdir()
    withenv("TEST_UNDECLARED_OUTPUTS_DIR" => dir) do
        @test run_benchmarks(["noop" => () -> 1]; seconds = 0.05) == 1
    end
    report = read(joinpath(dir, "dx-bench.json"), String)
    @test occursin("\"schema\":\"dx-bench/v1\"", report)
    @test occursin("\"name\":\"noop\"", report)
end
