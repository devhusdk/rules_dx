module Fixtures.Fuzz.Harness

open System.Text
open SharpFuzz

[<EntryPoint>]
let main _ =
    Fuzzer.LibFuzzer.Run(fun span -> Parser.parse (Encoding.UTF8.GetString(span)))
    0
