package hello;

import example.greet.Greeter;
import example.testhelper.TestHelper;
import org.junit.Test;

public class HelloTest {
  @Test public void testHello() { Greeter.greet("world"); TestHelper.check("world"); }
}
