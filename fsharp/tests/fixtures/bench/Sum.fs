namespace BenchFixture

open BenchmarkDotNet.Attributes

type SumBench() =
    [<Benchmark>]
    member _.Sum() =
        let mutable total = 0

        for i in 0..999 do
            total <- total + i

        total
