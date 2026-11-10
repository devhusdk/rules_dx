package quality.testdata.jvmclean;

/** Builds a greeting. */
public final class Clean {
  private Clean() {}

  /**
   * Returns the greeting for a name.
   *
   * @param name who to greet
   * @return the greeting
   */
  public static String hello(final String name) {
    return "hello " + name;
  }
}
