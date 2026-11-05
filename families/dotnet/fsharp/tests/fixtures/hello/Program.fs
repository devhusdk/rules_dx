// Seed F# binary; consumer of fsharp_binary.
module Program

[<EntryPoint>]
let main _ =
    printfn "%s" (Hello.greet "world")
    0
