// Java test whose test_class comes from the target name.
package hello;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class InferredTest {
  @Test
  public void testHello() {
    assertEquals("hello world", Hello.hello("world"));
  }
}
