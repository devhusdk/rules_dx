package billing;

import com.google.common.collect.ImmutableSortedMap;
import java.util.Locale;
import java.util.Map;
import java.util.SortedMap;
import java.util.TreeMap;

/** Sums invoices. */
public final class Totals {
  private Totals() {}

  /** Returns the grand total of all invoices in cents. */
  public static long all(Iterable<Invoice> invoices) {
    long sum = 0;
    for (Invoice invoice : invoices) {
      sum += invoice.total();
    }
    return sum;
  }

  /** Returns each customer's total in cents, sorted by customer. */
  public static SortedMap<String, Long> byCustomer(Iterable<Invoice> invoices) {
    Map<String, Long> sums = new TreeMap<>();
    for (Invoice invoice : invoices) {
      sums.merge(invoice.customer(), invoice.total(), Long::sum);
    }
    return ImmutableSortedMap.copyOf(sums);
  }

  /** Formats cents as a decimal amount. */
  public static String money(long cents) {
    return String.format(Locale.ROOT, "%d.%02d", cents / 100, cents % 100);
  }
}
