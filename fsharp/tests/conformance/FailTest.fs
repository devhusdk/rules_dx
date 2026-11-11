module FailTest

[<EntryPoint>]
let main _ =
    if Greet.greet "world" <> "goodbye world" then
        eprintfn "failing_case"
        1
    else
        0
