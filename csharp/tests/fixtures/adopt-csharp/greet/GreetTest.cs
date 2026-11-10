// Foreign C# test: handwritten owner of the test-owned source. The
// generator never emits csharp_test; `*Test.cs` files stay out of the
// generated library and this rule survives regeneration unchanged.
// Uses the pinned xunit.v3.assert lock (Xunit.Assert) without the MTP
// runner shims: the plain Main stays generation-stable while proving the
// Paket hub wiring.
using System;
using Xunit;

public static class GreetTestMain
{
    public static int Main(string[] args)
    {
        try
        {
            Assert.Equal("hello world", Greet.Greeter.SayHello("world"));
            Assert.Equal(" world", Greet.Helper.Suffix());
        }
        catch (Exception ex)
        {
            Console.WriteLine("FAIL: " + ex.Message);
            return 1;
        }
        Console.WriteLine("PASS");
        return 0;
    }
}
