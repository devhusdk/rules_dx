package conformance;

public final class PassTest {
  private PassTest() {}

  public static void main(String[] args) {
    if (!Greet.greet("world").equals("hello world")) {
      throw new AssertionError("passingCase");
    }
  }
}
