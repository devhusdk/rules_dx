package conformance;

public final class FailTest {
  private FailTest() {}

  public static void main(String[] args) {
    if (!Greet.greet("world").equals("goodbye world")) {
      throw new AssertionError("failingCase");
    }
  }
}
