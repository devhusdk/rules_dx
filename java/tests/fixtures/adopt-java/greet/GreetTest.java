package greet;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class GreetTest {
  @Test
  public void testGreet() {
    assertEquals("hello world", Greet.greet("world"));
  }

  @Test
  public void testSuffix() {
    assertEquals(" world", Helper.suffix());
  }

  @Test
  public void testGuavaJoin() {
    assertEquals("hello,world", Guava.join());
  }
}
