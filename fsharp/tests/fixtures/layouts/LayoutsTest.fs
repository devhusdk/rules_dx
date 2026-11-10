// F# layouts fixture test; plain executable (exit code is the verdict).
module LayoutsTest

open Layouts

[<EntryPoint>]
let main _ =
    let order = { item = { label = "tea" }; count = 2 }
    let got = sprintf "%d %s %s %d" (App.run ()) (Banner.show ()) order.item.label order.count
    if got <> "42 hello world tea 2" then
        printfn "FAIL: got '%s'" got
        1
    else
        printfn "PASS"
        0
