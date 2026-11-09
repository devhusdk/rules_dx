using Stats
using Test

@testset "stats" begin
    xs = [1.0, 2.0, 3.0, 4.0]
    @test mean(xs) == 2.5
    @test variance(xs) == 1.25
    @test quantile(xs, 0.0) == 1.0
    @test quantile(xs, 0.5) == 2.5
    @test quantile(xs, 1.0) == 4.0
    @test collect(keys(histogram([5.0, 1.0, 1.5, 12.0], 5))) == [0.0, 5.0, 10.0]
    @test collect(values(histogram([5.0, 1.0, 1.5, 12.0], 5))) == [2, 1, 1]
    @test_throws ArgumentError mean(Float64[])
    @test_throws ArgumentError mean([1.0, NaN])
    @test_throws ArgumentError quantile(xs, 2)
end
