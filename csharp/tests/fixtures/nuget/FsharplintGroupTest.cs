using System;

public static class FsharplintGroupTest
{
    public static int Main(string[] args)
    {
        if (new Newtonsoft.Json.JsonSerializerSettings() == null)
        {
            return 1;
        }
        Console.WriteLine("PASS");
        return 0;
    }
}
