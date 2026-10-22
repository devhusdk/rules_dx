package billing;

import static org.junit.Assert.assertEquals;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedMap;
import org.junit.Test;

public class TotalsTest {
  private static Invoice invoice(String id, String customer, int quantity, int cents) {
    return new Invoice(id, customer, ImmutableList.of(new LineItem("sku", quantity, cents)));
  }

  @Test
  public void sumsAllInvoices() {
    ImmutableList<Invoice> invoices =
        ImmutableList.of(invoice("1", "b", 2, 50), invoice("2", "a", 1, 25));
    assertEquals(125, Totals.all(invoices));
  }

  @Test
  public void groupsByCustomerInOrder() {
    ImmutableList<Invoice> invoices =
        ImmutableList.of(
            invoice("1", "b", 2, 50), invoice("2", "a", 1, 25), invoice("3", "b", 1, 1));
    ImmutableSortedMap<String, Long> sums = Totals.byCustomer(invoices);
    assertEquals(ImmutableList.of("a", "b"), sums.keySet().asList());
    assertEquals(Long.valueOf(101), sums.get("b"));
  }

  @Test
  public void largeLinesDoNotOverflow() {
    assertEquals(1_000_000_000_000L, invoice("1", "a", 1_000_000, 1_000_000).total());
  }

  @Test
  public void formatsMoney() {
    assertEquals("0.05", Totals.money(5));
    assertEquals("51.00", Totals.money(5100));
  }
}
