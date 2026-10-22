package billing;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import com.google.common.collect.ImmutableList;
import java.io.IOException;
import java.io.StringReader;
import org.junit.Test;

public class InvoiceReaderTest {
  private static final String ONE =
      "[{\"id\":\"A\",\"customer\":\"c\",\"items\":[{\"sku\":\"s\",\"quantity\":2,\"cents\":5}]}]";

  private static String message(String json) {
    InvoiceException e =
        assertThrows(InvoiceException.class, () -> InvoiceReader.read(new StringReader(json)));
    return e.getMessage();
  }

  @Test
  public void bundledResourceIsLoadedFromTheLibrary() throws Exception {
    ImmutableList<Invoice> invoices = InvoiceReader.bundled();
    assertEquals(3, invoices.size());
    assertEquals("INV-1001", invoices.get(0).id());
    assertEquals("washer-m6", invoices.get(0).items().get(1).sku());
    assertEquals(200, invoices.get(0).items().get(1).quantity());
  }

  @Test
  public void readsOneInvoice() throws Exception {
    ImmutableList<Invoice> invoices = InvoiceReader.read(new StringReader(ONE));
    assertEquals(1, invoices.size());
    assertEquals(10, invoices.get(0).total());
  }

  @Test
  public void generatedBuildInfoCarriesTheSchemaFile() {
    assertEquals(3, BuildInfo.SCHEMA);
  }

  @Test
  public void rejectsNegativeQuantity() {
    assertEquals(
        "quantity must be a whole number from 0 to 1000000",
        message(ONE.replace("\"quantity\":2", "\"quantity\":-1")));
  }

  @Test
  public void rejectsQuantityAboveTheLimit() {
    assertEquals(
        "quantity must be a whole number from 0 to 1000000",
        message(ONE.replace("\"quantity\":2", "\"quantity\":1000001")));
  }

  @Test
  public void rejectsFractionalPrice() {
    assertEquals(
        "cents must be a whole number from 0 to 1000000",
        message(ONE.replace("\"cents\":5", "\"cents\":5.5")));
  }

  @Test
  public void rejectsWrongType() {
    assertEquals(
        "quantity must be a number", message(ONE.replace("\"quantity\":2", "\"quantity\":\"2\"")));
  }

  @Test
  public void rejectsBlankCustomer() {
    assertEquals(
        "customer is blank", message(ONE.replace("\"customer\":\"c\"", "\"customer\":\" \"")));
  }

  @Test
  public void rejectsNonArray() {
    assertEquals("invoice file must hold an array", message("{}"));
  }

  @Test
  public void missingResourceIsReported() {
    IOException e =
        assertThrows(IOException.class, () -> InvoiceReader.fromResource("/billing/absent.json"));
    assertEquals("missing resource /billing/absent.json", e.getMessage());
  }

  @Test
  public void rejectsItemsThatAreNotAnArray() {
    assertEquals(
        "items must be an array", message("[{\"id\":\"A\",\"customer\":\"c\",\"items\":{}}]"));
  }

  @Test
  public void rejectsNonStringField() {
    assertEquals(
        "customer must be a string", message(ONE.replace("\"customer\":\"c\"", "\"customer\":3")));
  }

  @Test
  public void rejectsJsonThatOnlyALenientParserAccepts() {
    String[] lenient = {
      "[{'id':'A','customer':'c','items':[]}]",
      "[{id:\"A\",customer:\"c\",items:[]}]",
      "// note\n" + ONE,
      ONE + " []",
      "[{\"id\":\"A\",\"customer\":\"c\",\"items\":[]},]",
      "",
    };
    for (String json : lenient) {
      assertEquals(json, "invoice file is not valid JSON", message(json));
    }
  }

  @Test
  public void rejectsBrokenJson() {
    assertEquals("invoice file is not valid JSON", message("[{\"id\": "));
  }

  @Test
  public void rejectsNonObjectInvoice() {
    assertEquals("invoice must be an object", message("[1]"));
  }
}
