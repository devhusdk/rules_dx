using Starter
using Test

@testset "Starter" begin
    @test greet("world") == "hello world"
end
