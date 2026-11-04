package billing;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertTrue;

import com.google.common.collect.ImmutableList;
import com.google.gson.JsonParser;
import java.net.URISyntaxException;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.junit.Test;

public class ClasspathTest {
  private static Path jarOf(Class<?> type) throws URISyntaxException {
    return Paths.get(type.getProtectionDomain().getCodeSource().getLocation().toURI());
  }

  @Test
  public void ownedClassesComeFromTheBillingJar() throws Exception {
    assertTrue(jarOf(Invoice.class).getFileName().toString().startsWith("libbilling"));
    assertEquals(jarOf(Invoice.class), jarOf(InvoiceReader.class));
    assertEquals(jarOf(Invoice.class), jarOf(BuildInfo.class));
  }

  @Test
  public void dependencyClassesComeFromTheirOwnJars() throws Exception {
    assertNotEquals(jarOf(Invoice.class), jarOf(JsonParser.class));
    assertNotEquals(jarOf(Invoice.class), jarOf(ImmutableList.class));
    assertTrue(jarOf(JsonParser.class).getFileName().toString().contains("gson"));
    assertTrue(jarOf(ImmutableList.class).getFileName().toString().contains("guava"));
  }
}
