package spotbugs_owned;

public class Dirty {
  public static int length(boolean flag) {
    String value = flag ? null : "x";
    if (value == null) {
      return value.length();
    }
    return value.length();
  }

  @SuppressWarnings("fallthrough")
  int sw(int x) {
    int r = 0;
    switch (x) {
      case 1:
        r = 1;
      case 2:
        r = 2;
        break;
      default:
        break;
    }
    return r;
  }
}
