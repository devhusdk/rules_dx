package billing;

/** One invoiced line. */
public final class LineItem {
  private final String sku;
  private final int quantity;
  private final int cents;

  /** Creates a line. */
  public LineItem(String sku, int quantity, int cents) {
    this.sku = sku;
    this.quantity = quantity;
    this.cents = cents;
  }

  /** Returns the item code. */
  public String sku() {
    return sku;
  }

  /** Returns the number of units. */
  public int quantity() {
    return quantity;
  }

  /** Returns the unit price in cents. */
  public int cents() {
    return cents;
  }

  /** Returns quantity times unit price in cents. */
  public long total() {
    return (long) quantity * cents;
  }
}
