package spotbugs_owned;

public class UsesApi implements Api {
  @Override
  public int size() {
    return 0;
  }

  public static int sizeOf(Api api) {
    return api.size();
  }
}
