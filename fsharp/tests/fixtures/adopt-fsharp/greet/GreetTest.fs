// Foreign F# test: handwritten owner of the test-owned source. The
// generator never emits fsharp_test; `*Test.fs` files stay out of the
// generated library and this rule survives regeneration unchanged.
module GreetTest

[<EntryPoint>]
let main _ =
    let got = Greet.Greeter.SayHello("world")
    if got <> "hello world" then
        printfn "FAIL: got '%s'" got
        1
    elif Greet.Helper.Suffix() <> " world" then
        printfn "FAIL: suffix mismatch"
        1
    else
        printfn "PASS"
        0
