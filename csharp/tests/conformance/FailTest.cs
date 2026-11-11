using System;

public static class FailTest
{
    public static int Main()
    {
        if (Conformance.Greeter.Greet("world") != "goodbye world")
        {
            Console.Error.WriteLine("failing_case");
            return 1;
        }
        return 0;
    }
}
