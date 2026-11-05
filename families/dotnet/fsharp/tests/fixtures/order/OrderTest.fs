// F# compile-order fixture test; plain executable (exit code is the verdict).
module OrderTest

[<EntryPoint>]
let main _ =
    let got = Demo.greet "world"
    if got <> "hello world" then
        printfn "FAIL: got '%s'" got
        1
    else
        printfn "PASS"
        0
