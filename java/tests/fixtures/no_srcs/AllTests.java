// Seed Java test run by a java_test with no srcs of its own.
package nosrcs;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class AllTests {
  @Test
  public void testAnswer() {
    assertEquals(42, Impl.answer());
  }
}
