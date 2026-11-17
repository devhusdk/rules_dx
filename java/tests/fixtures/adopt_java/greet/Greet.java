// Foreign Java greeting implementation: adopted without upstream changes.
package greet;

import java.util.Objects;

public final class Greet {
  private Greet() {}

  public static String greet(String name) {
    return "hello " + Objects.requireNonNull(name);
  }
}
