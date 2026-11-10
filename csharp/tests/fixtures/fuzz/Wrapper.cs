namespace Fixtures.Fuzz;

public static class Wrapper
{
    public static void Handle(string text)
    {
        Parser.Parse(text);
    }
}
