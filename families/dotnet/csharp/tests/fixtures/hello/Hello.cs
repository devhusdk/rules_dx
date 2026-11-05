// Seed C# library; consumer of csharp_library.
namespace Hello;

public static class Greeter
{
    public static string Greet(string name)
    {
        return "hello " + name;
    }
}
