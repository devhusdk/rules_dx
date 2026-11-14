module FsharplintGroupTest

[<EntryPoint>]
let main _ =
    if isNull (box (Newtonsoft.Json.JsonSerializerSettings())) then
        1
    else
        printfn "PASS"
        0
