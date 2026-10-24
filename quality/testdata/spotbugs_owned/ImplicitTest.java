package spotbugs_owned;

import org.junit.Assert;
import org.junit.Test;

public class ImplicitTest {
  @Test
  public void sizeIsZero() {
    Assert.assertEquals(0, Clean.size());
  }
}
