// Seed C# Roslyn aggregation fixture.
namespace Fixtures.Roslyn;

public class Sample
{
    public string Greet(string name)
    {
        // Shared finding in every pivot: CA1822 (member does not access
        // instance data) fires on `Greet` under both net8.0 and net10.0.
        return "hello " + name;
    }

#if NET10_0
    public void Log(string message)
    {
        // TFM-specific finding: CA1303 (do not pass literals as localized
        // parameters) fires only in the net10.0 pivot where this block is
        // compiled. A single-SARIF reader that keeps only one pivot would
        // silently drop it, so aggregation must be a union, never a pick-one.
        System.Console.WriteLine(message);
    }
#endif
}
