package billing;

import com.google.common.collect.ImmutableList;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;

/** Reads invoices from JSON. */
public final class InvoiceReader {
  /** Largest accepted quantity or unit price. */
  public static final int MAX = 1_000_000;

  private static final String BUNDLED = "/billing/invoices.json";

  private InvoiceReader() {}

  /** Reads the invoices packaged with this library. */
  public static ImmutableList<Invoice> bundled() throws InvoiceException, IOException {
    try (InputStream in = InvoiceReader.class.getResourceAsStream(BUNDLED)) {
      if (in == null) {
        throw new IOException("missing resource " + BUNDLED);
      }
      return read(new InputStreamReader(in, StandardCharsets.UTF_8));
    }
  }

  /** Parses a JSON array of invoices. */
  public static ImmutableList<Invoice> read(Reader reader) throws InvoiceException {
    JsonElement root;
    try {
      root = JsonParser.parseReader(reader);
    } catch (JsonParseException e) {
      throw new InvoiceException("invoice file is not valid JSON", e);
    }
    if (!root.isJsonArray()) {
      throw new InvoiceException("invoice file must hold an array");
    }
    ImmutableList.Builder<Invoice> invoices = ImmutableList.builder();
    for (JsonElement element : root.getAsJsonArray()) {
      invoices.add(invoice(object(element, "invoice")));
    }
    return invoices.build();
  }

  private static Invoice invoice(JsonObject json) throws InvoiceException {
    ImmutableList.Builder<LineItem> items = ImmutableList.builder();
    JsonElement lines = json.get("items");
    if (lines == null || !lines.isJsonArray()) {
      throw new InvoiceException("items must be an array");
    }
    for (JsonElement line : lines.getAsJsonArray()) {
      JsonObject item = object(line, "item");
      items.add(new LineItem(text(item, "sku"), whole(item, "quantity"), whole(item, "cents")));
    }
    try {
      return new Invoice(text(json, "id"), text(json, "customer"), items.build());
    } catch (IllegalArgumentException e) {
      throw new InvoiceException(e.getMessage(), e);
    }
  }

  private static JsonObject object(JsonElement element, String what) throws InvoiceException {
    if (!element.isJsonObject()) {
      throw new InvoiceException(what + " must be an object");
    }
    return element.getAsJsonObject();
  }

  private static String text(JsonObject json, String key) throws InvoiceException {
    JsonElement value = json.get(key);
    if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
      throw new InvoiceException(key + " must be a string");
    }
    return value.getAsString();
  }

  private static int whole(JsonObject json, String key) throws InvoiceException {
    JsonElement value = json.get(key);
    if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()) {
      throw new InvoiceException(key + " must be a number");
    }
    BigDecimal number = value.getAsBigDecimal();
    try {
      int whole = number.intValueExact();
      if (whole >= 0 && whole <= MAX) {
        return whole;
      }
    } catch (ArithmeticException e) {
      throw new InvoiceException(key + " must be a whole number from 0 to " + MAX, e);
    }
    throw new InvoiceException(key + " must be a whole number from 0 to " + MAX);
  }
}
