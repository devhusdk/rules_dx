using System;

public static class LockMain
{
    public static void Main(string[] args)
    {
        var greeting = Google.Protobuf.ByteString.CopyFromUtf8("hello lock");
        Console.WriteLine(greeting.ToStringUtf8());
    }
}
