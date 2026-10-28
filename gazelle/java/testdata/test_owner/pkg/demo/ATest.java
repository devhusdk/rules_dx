package demo;

import static org.junit.Assert.assertEquals;
import org.junit.Test;

public class ATest {
  @Test
  public void testGreet() {
    assertEquals("hello world", Demo.greet("world"));
  }
}
