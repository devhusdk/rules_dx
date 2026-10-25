// Foreign Java helper backed by guava from the shared @maven hub.
package greet;

import com.google.common.collect.ImmutableList;

public final class Guava {
  private Guava() {}

  public static String join() {
    return String.join(",", ImmutableList.of("hello", "world"));
  }
}
