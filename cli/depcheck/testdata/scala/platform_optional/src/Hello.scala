package hello

import example.greet.Greeter
import example.optionalfeat.OptionalFeat

public class Hello {
  public static String hello(String name) {
    return Greeter.greet(name) + OptionalFeat.label()
  }
}
