namespace Demo

open System.Collections.Generic

type Demo =
    static member Greet(name: string) = "hello " + name + Helper.Suffix()
