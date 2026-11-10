using System.Text;
using Fixtures.Fuzz;
using SharpFuzz;

namespace Fixtures.Fuzz;

public static class Harness
{
    public static void Main()
    {
        Fuzzer.LibFuzzer.Run(span => Wrapper.Handle(Encoding.UTF8.GetString(span)));
    }
}
