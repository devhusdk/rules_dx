package conformance;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public final class PassTest {
  @Test
  public void passingCase() {
    assertEquals("hello world", Greet.greet("world"));
  }
}
