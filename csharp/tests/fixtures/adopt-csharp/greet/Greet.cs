// Foreign C# greeting implementation: adopted without upstream changes.
namespace Greet;

using System;

public static class Greeter
{
    public static string SayHello(string name)
    {
        return "hello " + name ?? throw new ArgumentNullException(nameof(name));
    }
}
