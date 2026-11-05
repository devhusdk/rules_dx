package hello

import example.greet.Greeter

public class Hello {
  public static String hello(String name) {
    return Greeter.greet(name)
  }
}
