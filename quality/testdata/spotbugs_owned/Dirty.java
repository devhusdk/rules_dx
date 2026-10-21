package spotbugs_owned;

public class Dirty {
  public static int length(boolean flag) {
    String value = flag ? null : "x";
    if (value == null) {
      return value.length();
    }
    return value.length();
  }
}
