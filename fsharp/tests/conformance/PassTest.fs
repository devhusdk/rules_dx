module PassTest

[<EntryPoint>]
let main _ =
    if Greet.greet "world" <> "hello world" then 1 else 0
