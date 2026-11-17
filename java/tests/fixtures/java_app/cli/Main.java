package cli;

/** Entry point for the invoice command. */
public final class Main {
  private Main() {}

  /** Runs the command and exits with its code. */
  public static void main(String[] args) {
    System.exit(Cli.run(args, System.getenv(), System.out, System.err));
  }
}
