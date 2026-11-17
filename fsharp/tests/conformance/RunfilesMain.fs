module RunfilesMain

open System.IO

[<EntryPoint>]
let main args =
    let runfiles = Bazel.Runfiles.Create()
    printf "%s" (File.ReadAllText(runfiles.Rlocation(args[0])))
    0
