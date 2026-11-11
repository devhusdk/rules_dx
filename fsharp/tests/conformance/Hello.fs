module Hello

[<EntryPoint>]
let main _ =
    printfn "%s" (Greet.greet "world")
    0
