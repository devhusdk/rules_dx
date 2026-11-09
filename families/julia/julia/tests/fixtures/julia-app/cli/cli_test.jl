import Runfiles
using Test

binary = Runfiles.rlocation(ENV["JULIA_APP_BINARY"])
digits = parse(Int, ENV["JULIA_APP_DIGITS"])

@testset "cli" begin
    @test binary !== nothing
    out = IOBuffer()
    proc = run(pipeline(`$binary`; stdout = out); wait = true)
    text = String(take!(out))
    @test success(proc)
    @test occursin("readings: 8\n", text)
    @test occursin("mean: $(round(12.71875; digits = digits))\n", text)
    @test occursin("archive: ok\n", text)
end
