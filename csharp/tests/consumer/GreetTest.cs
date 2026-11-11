using System;

public static class GreetTest
{
    public static int Main(string[] args)
    {
        if (Consumer.Greeter.Greet("world") != "hello world")
        {
            return 1;
        }
        Console.WriteLine("PASS");
        return 0;
    }
}
