// Foreign Java stdlib-only unit: adopted without upstream changes.
package solo;

public final class Pure {
  private Pure() {}

  public static String pure(String name) {
    return "hello " + name;
  }
}
