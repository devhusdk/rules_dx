using Archive
using Test

@testset "archive" begin
    data = Vector{UInt8}(repeat("sensor,reading\n", 200))
    packed = compress(data)
    @test length(packed) < length(data)
    @test decompress(packed, length(data)) == data

    bad = copy(packed)
    bad[20] = xor(bad[20], 0xff)
    @test_throws ErrorException decompress(bad, length(data))
    @test_throws ErrorException decompress(UInt8[1, 2, 3], 16)
end
