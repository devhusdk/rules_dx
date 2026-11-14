using System;

public static class MainGroupTest
{
    public static int Main(string[] args)
    {
        if (Google.Protobuf.ByteString.Empty == null)
        {
            return 1;
        }
        Console.WriteLine("PASS");
        return 0;
    }
}
