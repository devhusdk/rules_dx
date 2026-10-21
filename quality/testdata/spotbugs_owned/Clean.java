package spotbugs_owned;

public class Clean {
  public static int size() {
    return Dirty.length(false);
  }
}
