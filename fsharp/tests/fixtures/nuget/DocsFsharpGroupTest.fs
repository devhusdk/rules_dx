module DocsFsharpGroupTest

[<EntryPoint>]
let main _ =
    if isNull (box (System.Collections.Immutable.ImmutableArray<int>.Empty)) then
        1
    else
        printfn "PASS"
        0
