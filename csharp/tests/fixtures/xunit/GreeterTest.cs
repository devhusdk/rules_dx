// Seed C# xUnit test: runs via csharp_test over the pinned
// xunit.v3 4.0.0 plus xunit.analyzers 2.0.0 Paket lock.
using Xunit;

namespace XunitFixture;

public class GreeterTests
{
    [Fact]
    public void GreetReturnsHelloWorld()
    {
        Assert.Equal("hello world", Greeter.Greet("world"));
    }

    [Theory]
    [InlineData("world", "hello world")]
    [InlineData("xunit", "hello xunit")]
    public void GreetGreetsByName(string name, string expected)
    {
        Assert.Equal(expected, Greeter.Greet(name));
    }
}
