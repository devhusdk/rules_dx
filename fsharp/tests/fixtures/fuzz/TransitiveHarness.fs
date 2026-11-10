module Fixtures.Fuzz.TransitiveHarness

open System.Text
open SharpFuzz

[<EntryPoint>]
let main _ =
    Fuzzer.LibFuzzer.Run(fun span -> Wrapper.handle (Encoding.UTF8.GetString(span)))
    0
