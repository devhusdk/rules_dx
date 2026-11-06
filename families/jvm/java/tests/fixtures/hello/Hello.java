// Seed Java library; consumer of java_library.
package hello;

public final class Hello {
  private Hello() {}

  public static String hello(String name) {
    return "hello " + name;
  }
}
