using System;

public static class DocsCsharpGroupTest
{
    public static int Main(string[] args)
    {
        if (new Markdig.MarkdownPipelineBuilder() == null)
        {
            return 1;
        }
        Console.WriteLine("PASS");
        return 0;
    }
}
