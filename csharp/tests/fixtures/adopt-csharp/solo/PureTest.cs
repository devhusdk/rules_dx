// Foreign C# test: handwritten owner of the test-owned source. Uses the
// pinned xunit.v3.assert lock (Xunit.Assert) without MTP shims.
using System;
using Xunit;

public static class PureTestMain
{
    public static int Main(string[] args)
    {
        try
        {
            Assert.Equal("hello world", Solo.Pure.SayHello("world"));
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
