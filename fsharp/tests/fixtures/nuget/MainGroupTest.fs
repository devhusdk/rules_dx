module MainGroupTest

[<EntryPoint>]
let main _ =
    if isNull (box (Google.Protobuf.ByteString.Empty)) then
        1
    else
        printfn "PASS"
        0
