// Seed F# xUnit test: runs via fsharp_test over the pinned
// xunit.v3 4.0.0 plus xunit.analyzers 2.0.0 Paket lock.
module GreeterTests

open Xunit

[<Fact>]
let ``Greet returns hello world`` () =
    Assert.Equal("hello world", Greeter.greet "world")

[<Theory>]
[<InlineData("world", "hello world")>]
[<InlineData("xunit", "hello xunit")>]
let ``Greet greets by name`` (name: string, expected: string) =
    Assert.Equal(expected, Greeter.greet name)
