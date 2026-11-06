// Seed Java test; consumer of java_test.
package hello;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class HelloTest {
  @Test
  public void testHello() {
    assertEquals("hello world", Hello.hello("world"));
  }
}
