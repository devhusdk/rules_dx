// Seed C# test; consumer of csharp_test (plain executable: exit
// code is the verdict; the xUnit/NUnit runner selection stays open under
// ADR 0019).
using System;

public static class HelloTestMain
{
    public static int Main(string[] args)
    {
        var got = Hello.Greeter.Greet("world");
        if (got != "hello world")
        {
            Console.WriteLine("FAIL: got '" + got + "'");
            return 1;
        }
        Console.WriteLine("PASS");
        return 0;
    }
}
