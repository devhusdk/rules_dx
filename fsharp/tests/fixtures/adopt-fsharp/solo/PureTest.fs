// Foreign F# test: handwritten owner of the test-owned source.
module PureTest

[<EntryPoint>]
let main _ =
    let got = Solo.Pure.SayHello("world")
    if got <> "hello world" then
        printfn "FAIL: got '%s'" got
        1
    else
        printfn "PASS"
        0
