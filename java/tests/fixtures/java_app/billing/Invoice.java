package billing;

import static com.google.common.base.Preconditions.checkArgument;

import com.google.common.collect.ImmutableList;
import java.util.List;

/** An invoice with its lines. */
public final class Invoice {
  private final String id;
  private final String customer;
  private final List<LineItem> items;

  /** Creates an invoice and rejects blank ids and customers. */
  public Invoice(String id, String customer, List<LineItem> items) {
    checkArgument(!id.trim().isEmpty(), "id is blank");
    checkArgument(!customer.trim().isEmpty(), "customer is blank");
    this.id = id;
    this.customer = customer;
    this.items = ImmutableList.copyOf(items);
  }

  /** Returns the invoice id. */
  public String id() {
    return id;
  }

  /** Returns the customer name. */
  public String customer() {
    return customer;
  }

  /** Returns the lines. */
  public List<LineItem> items() {
    return items;
  }

  /** Returns the sum of all lines in cents. */
  public long total() {
    long sum = 0;
    for (LineItem item : items) {
      sum += item.total();
    }
    return sum;
  }
}
