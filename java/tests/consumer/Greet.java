package consumer;

public final class Greet {
  private Greet() {}

  public static String greet(String name) {
    return "hello " + name;
  }
}
