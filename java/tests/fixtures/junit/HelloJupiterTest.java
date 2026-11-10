// Seed Java Jupiter test; consumer of the qualified JUnit 6.1.3 runner.
package hello;

import static org.junit.jupiter.api.Assertions.assertEquals;
import org.junit.jupiter.api.Test;

public class HelloJupiterTest {
  @Test
  public void testHello() {
    assertEquals("hello world", Hello.hello("world"));
  }
}
