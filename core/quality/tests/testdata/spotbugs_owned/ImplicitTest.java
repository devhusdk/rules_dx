package spotbugs_owned;

import org.junit.Assert;
import org.junit.Test;

public class ImplicitTest {
  @Test
  public void sizeIsOne() {
    Assert.assertEquals(1, Clean.size());
  }
}
