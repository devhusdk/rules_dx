using Greet
using Test

@testset "greeting" begin
    @test greeting("julia") == "Hello, julia!"
end
