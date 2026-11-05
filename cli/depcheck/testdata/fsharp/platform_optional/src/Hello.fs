open Greet
open OptionalFeat

namespace Hello {
  public static class Greeter {
    public static string HelloName(string name) { return GreeterLib.Greet(name) + OptionalFeatLib.Label() }
  }
}
