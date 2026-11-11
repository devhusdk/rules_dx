package conformance;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public final class FailTest {
  @Test
  public void failingCase() {
    assertEquals("goodbye world", Greet.greet("world"));
  }
}
