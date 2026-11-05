using Greet;
using OptionalFeat;

namespace Hello {
  public static class Greeter {
    public static string HelloName(string name) { return GreeterLib.Greet(name) + OptionalFeatLib.Label(); }
  }
}
