// Seed Java library behind an exports-only java_library.
package nosrcs;

public final class Impl {
  private Impl() {}

  public static int answer() {
    return 42;
  }
}
