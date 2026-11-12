package conformance;

import org.junit.Assert;

public final class LockMain {
  private LockMain() {}

  public static void main(String[] args) {
    String greeting = "hello lock";
    Assert.assertTrue(greeting.startsWith("hello"));
    System.out.println(greeting);
  }
}
