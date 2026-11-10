namespace Fixtures.Fuzz;

public static class Parser
{
    public static void Parse(string text)
    {
        if (text.Length > 2 && text[0] == 'F' && text[1] == 'U' && text[2] == 'Z')
        {
            throw new System.InvalidOperationException("parser crash");
        }
    }
}
