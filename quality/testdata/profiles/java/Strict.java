package quality.testdata.profiles.java;

/** Fixture with one finding that only the strict profiles report for each Java tool. */
public final class Strict {
  private Strict() {}

  /**
   * Returns a fixed timeout.
   *
   * @return the timeout in milliseconds
   */
  public static int timeout() {
    return 4242;
  }

  /**
   * Returns the length of the text, or zero when the text is missing.
   *
   * @param text the text to measure
   * @return the length of the text
   */
  public static int length(String text) {
    try {
      return text.length();
    } catch (NullPointerException e) {
      return 0;
    }
  }

  /**
   * Compares the text to a name.
   *
   * @param text the text to compare
   * @return whether the text matches ignoring case
   */
  public static boolean isName(String text) {
    return text.toUpperCase().equals("NAME");
  }
}
