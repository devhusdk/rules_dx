open Greet
open TestHelper

public static class HelloTest {
  public static void Test() { GreeterLib.Greet("world") TestHelperLib.Check("world") }
}
