// Foreign F# greeting implementation: adopted without upstream changes.
namespace Greet

open System

type Greeter =
    static member SayHello(name: string) =
        if isNull (box name) then raise (ArgumentNullException(nameof name))
        "hello " + name
