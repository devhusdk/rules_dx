using Demo
using Test

include("helpers.jl")

@test Demo.norm2([3.0, 4.0]) == 5.0
