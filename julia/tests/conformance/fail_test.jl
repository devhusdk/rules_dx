using Greet
using Test

@testset "failing case" begin
    @test greet("world") == "goodbye world"
end
