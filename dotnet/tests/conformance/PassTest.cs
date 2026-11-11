using System;

public static class PassTest
{
    public static int Main()
    {
        if (Conformance.Greeter.Greet("world") != "hello world")
        {
            return 1;
        }
        return 0;
    }
}
