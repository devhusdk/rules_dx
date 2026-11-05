open Greet
open TestHelper

namespace Hello {
  public static class Greeter {
    public static string HelloName(string name) { return GreeterLib.Greet(name) + TestHelperLib.Check(name) }
  }
}
