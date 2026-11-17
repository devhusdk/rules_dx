// Foreign F# stdlib-only unit: adopted without upstream changes.
namespace Solo

type Pure =
    static member SayHello(name: string) = "hello " + name
