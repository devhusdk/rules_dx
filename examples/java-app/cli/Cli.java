package cli;

import billing.BuildInfo;
import billing.Invoice;
import billing.InvoiceException;
import billing.InvoiceReader;
import billing.LineItem;
import billing.Totals;
import com.google.common.collect.ImmutableList;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Map;

/** Command line front end for the invoice library. */
public final class Cli {
  /** Success. */
  public static final int OK = 0;

  /** Bad command line. */
  public static final int USAGE = 2;

  /** Unreadable or malformed invoice file. */
  public static final int BAD_FILE = 3;

  /** Unknown invoice id. */
  public static final int NOT_FOUND = 4;

  private static final String USAGE_TEXT =
      "usage: invoice [--file FILE] list | total | customers | schema | show ID";

  private Cli() {}

  /** Runs one command and returns its exit code. */
  public static int run(String[] args, Map<String, String> env, PrintStream out, PrintStream err) {
    String file = env.get("INVOICE_FILE");
    int index = 0;
    if (args.length >= 2 && args[0].equals("--file")) {
      file = args[1];
      index = 2;
    }
    if (index >= args.length) {
      err.println(USAGE_TEXT);
      return USAGE;
    }
    String command = args[index];
    int rest = args.length - index - 1;
    if (command.equals("schema") && rest == 0) {
      out.println("schema " + BuildInfo.SCHEMA);
      return OK;
    }
    boolean known =
        (rest == 0
                && (command.equals("list")
                    || command.equals("total")
                    || command.equals("customers")))
            || (rest == 1 && command.equals("show"));
    if (!known) {
      err.println(USAGE_TEXT);
      return USAGE;
    }
    ImmutableList<Invoice> invoices;
    try {
      invoices = load(file, env.get("BUILD_WORKING_DIRECTORY"));
    } catch (IOException e) {
      err.println("invoice: cannot read invoice file");
      return BAD_FILE;
    } catch (InvoiceException e) {
      err.println("invoice: " + e.getMessage());
      return BAD_FILE;
    }
    switch (command) {
      case "list":
        for (Invoice invoice : invoices) {
          out.println(
              invoice.id()
                  + " "
                  + invoice.customer()
                  + " "
                  + invoice.items().size()
                  + " items "
                  + Totals.money(invoice.total()));
        }
        return OK;
      case "total":
        out.println("total " + Totals.money(Totals.all(invoices)));
        return OK;
      case "customers":
        Totals.byCustomer(invoices)
            .forEach((customer, cents) -> out.println(customer + " " + Totals.money(cents)));
        return OK;
      default:
        return show(invoices, args[index + 1], out, err);
    }
  }

  private static int show(
      ImmutableList<Invoice> invoices, String id, PrintStream out, PrintStream err) {
    for (Invoice invoice : invoices) {
      if (invoice.id().equals(id)) {
        for (LineItem item : invoice.items()) {
          out.println(
              item.sku()
                  + " "
                  + item.quantity()
                  + " x "
                  + Totals.money(item.cents())
                  + " = "
                  + Totals.money(item.total()));
        }
        out.println("total " + Totals.money(invoice.total()));
        return OK;
      }
    }
    err.println("invoice: no invoice " + id);
    return NOT_FOUND;
  }

  private static ImmutableList<Invoice> load(String file, String workingDir)
      throws IOException, InvoiceException {
    if (file == null) {
      return InvoiceReader.bundled();
    }
    Path path = Paths.get(file);
    if (!path.isAbsolute() && workingDir != null) {
      path = Paths.get(workingDir).resolve(path);
    }
    try (InputStream in = Files.newInputStream(path);
        Reader reader = new InputStreamReader(in, StandardCharsets.UTF_8)) {
      return InvoiceReader.read(reader);
    }
  }
}
