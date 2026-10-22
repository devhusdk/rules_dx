package cli;

import static org.junit.Assert.assertEquals;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.TemporaryFolder;

public class CliTest {
  @Rule public TemporaryFolder temp = new TemporaryFolder();

  private final ByteArrayOutputStream out = new ByteArrayOutputStream();
  private final ByteArrayOutputStream err = new ByteArrayOutputStream();

  private int run(Map<String, String> env, String... args) {
    return Cli.run(
        args,
        env,
        new PrintStream(out, true, StandardCharsets.UTF_8),
        new PrintStream(err, true, StandardCharsets.UTF_8));
  }

  private String stdout() {
    return out.toString(StandardCharsets.UTF_8);
  }

  private String stderr() {
    return err.toString(StandardCharsets.UTF_8);
  }

  @Test
  public void listReadsTheBundledInvoices() {
    assertEquals(Cli.OK, run(Map.of(), "list"));
    assertEquals(
        "INV-1001 acme 2 items 51.00\nINV-1002 globex 1 items 11.00\nINV-1003 acme 1 items 6.00\n",
        stdout());
  }

  @Test
  public void totalAndCustomers() {
    assertEquals(Cli.OK, run(Map.of(), "total"));
    assertEquals("total 68.00\n", stdout());
    out.reset();
    assertEquals(Cli.OK, run(Map.of(), "customers"));
    assertEquals("acme 57.00\nglobex 11.00\n", stdout());
  }

  @Test
  public void showPrintsLines() {
    assertEquals(Cli.OK, run(Map.of(), "show", "INV-1001"));
    assertEquals(
        "bolt-m6 100 x 0.35 = 35.00\nwasher-m6 200 x 0.08 = 16.00\ntotal 51.00\n", stdout());
  }

  @Test
  public void schemaComesFromTheGeneratedClass() {
    assertEquals(Cli.OK, run(Map.of(), "schema"));
    assertEquals("schema 3\n", stdout());
  }

  @Test
  public void unknownInvoiceExits4() {
    assertEquals(Cli.NOT_FOUND, run(Map.of(), "show", "INV-9"));
    assertEquals("invoice: no invoice INV-9\n", stderr());
  }

  @Test
  public void badUsageExits2() {
    assertEquals(Cli.USAGE, run(Map.of()));
    assertEquals(Cli.USAGE, run(Map.of(), "sell"));
    assertEquals(Cli.USAGE, run(Map.of(), "show"));
    assertEquals(Cli.USAGE, run(Map.of(), "--file"));
  }

  @Test
  public void fileFlagAndEnvSelectTheInput() throws Exception {
    Path file = temp.newFile("one.json").toPath();
    Files.writeString(
        file,
        "[{\"id\":\"X\",\"customer\":\"c\",\"items\":[{\"sku\":\"s\",\"quantity\":1,\"cents\":250}]}]");
    assertEquals(Cli.OK, run(Map.of(), "--file", file.toString(), "total"));
    assertEquals("total 2.50\n", stdout());
    out.reset();
    assertEquals(Cli.OK, run(Map.of("INVOICE_FILE", file.toString()), "total"));
    assertEquals("total 2.50\n", stdout());
  }

  @Test
  public void relativeFileResolvesAgainstTheInvokingDirectory() throws Exception {
    Path file = temp.newFile("rel.json").toPath();
    Files.writeString(file, "[]");
    Map<String, String> env = Map.of("BUILD_WORKING_DIRECTORY", temp.getRoot().toString());
    assertEquals(Cli.OK, run(env, "--file", "rel.json", "total"));
    assertEquals("total 0.00\n", stdout());
  }

  @Test
  public void missingFileExits3() {
    assertEquals(Cli.BAD_FILE, run(Map.of(), "--file", "/nonexistent/x.json", "list"));
    assertEquals("invoice: cannot read invoice file\n", stderr());
  }

  @Test
  public void malformedFileExits3() throws Exception {
    Path file = temp.newFile("bad.json").toPath();
    Files.writeString(
        file,
        "[{\"id\":\"X\",\"customer\":\"c\",\"items\":[{\"sku\":\"s\",\"quantity\":-4,\"cents\":1}]}]");
    assertEquals(Cli.BAD_FILE, run(Map.of(), "--file", file.toString(), "list"));
    assertEquals("invoice: quantity must be a whole number from 0 to 1000000\n", stderr());
  }
}
