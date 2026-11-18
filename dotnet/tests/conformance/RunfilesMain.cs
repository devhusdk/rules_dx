using System;
using System.IO;

public static class RunfilesMain
{
    public static void Main(string[] args)
    {
        var runfiles = Bazel.Runfiles.Create();
        Console.Write(File.ReadAllText(runfiles.Rlocation(args[0])));
    }
}
