package billing;

/** Raised for input that is not a valid invoice list. */
public final class InvoiceException extends Exception {
  private static final long serialVersionUID = 1L;

  /** Creates an exception with a message. */
  public InvoiceException(String message) {
    super(message);
  }

  /** Creates an exception with a message and cause. */
  public InvoiceException(String message, Throwable cause) {
    super(message, cause);
  }
}
