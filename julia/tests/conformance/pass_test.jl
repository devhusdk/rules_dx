using Greet
using Test

@testset "greets by name" begin
    @test greet("world") == "hello world"
end
