package conformance;

public final class LintDirty {
  @SuppressWarnings("UnusedMethod") // Error Prone off; PMD must still report this method.
  private static void helper() {}

  public static String hello(String name) {
    return "hello " + name;
  }
}
