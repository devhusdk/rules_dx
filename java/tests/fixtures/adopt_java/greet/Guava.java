// Foreign Java guava helper: one pinned module dep (guava 32.0.1-jre).
// Generation resolves `ImmutableList` through `maven_install.json` to the
// shared @maven hub; the pom above records the coordinates.
package greet;

import com.google.common.collect.ImmutableList;

public final class Guava {
  private Guava() {}

  public static String join() {
    return String.join(",", ImmutableList.of("hello", "world"));
  }
}
