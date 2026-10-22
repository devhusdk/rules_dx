package cli;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class ConsumerChoiceTest {
  @Test
  public void consumerJvmOptionIsKept() {
    assertEquals("kept", System.getProperty("dx.consumer.choice"));
  }
}
