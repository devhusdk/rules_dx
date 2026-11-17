module LockMain

[<EntryPoint>]
let main _ =
    let greeting = Google.Protobuf.ByteString.CopyFromUtf8 "hello lock"
    printfn "%s" (greeting.ToStringUtf8())
    0
