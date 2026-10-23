package diagnostics;

import com.google.gson.JsonParser;

/** Needs gson on the classpath. */
public final class MissingDep {
  private MissingDep() {}

  /** Parses a JSON array and returns its size. */
  public static int size(String json) {
    return JsonParser.parseString(json).getAsJsonArray().size();
  }
}
