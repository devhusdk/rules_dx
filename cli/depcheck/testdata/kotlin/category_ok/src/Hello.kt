package hello

import example.greet.Greeter
import example.testhelper.TestHelper

public class Hello {
  public static String hello(String name) { return Greeter.greet(name) + TestHelper.check(name) }
}
