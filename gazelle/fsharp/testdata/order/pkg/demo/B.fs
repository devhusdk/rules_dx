namespace Demo

open System.Collections.Generic

type B =
    static member Value() = C.Value() + "b"
