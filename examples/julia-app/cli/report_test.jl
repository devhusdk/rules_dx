using Report
using Test

function capture(path, digits)
    out = IOBuffer()
    err = IOBuffer()
    code = run_report(path, digits; io = out, err = err)
    return code, String(take!(out)), String(take!(err))
end

mktempdir() do dir
    good = joinpath(dir, "good.csv")
    write(good, "sensor,value\na,1.0\nb,2.0\nc,3.0\n")
    bad = joinpath(dir, "bad.csv")
    write(bad, "sensor,value\na,1.0\nb,oops\n")
    empty = joinpath(dir, "empty.csv")
    write(empty, "sensor,value\n")

    @testset "report" begin
        code, out, err = capture(good, 2)
        @test code == 0
        @test err == ""
        @test occursin("readings: 3\n", out)
        @test occursin("mean: 2.0\n", out)
        @test occursin("archive: ok\n", out)

        code, out, err = capture(bad, 2)
        @test code == 1
        @test out == ""
        @test occursin("line 3: malformed row", err)

        code, _, err = capture(empty, 2)
        @test code == 1
        @test occursin("no readings", err)

        code, _, err = capture(joinpath(dir, "missing.csv"), 2)
        @test code == 2
        @test occursin("cannot read", err)
    end
end
