package org.example.adoptandroid;

public final class Report {
  private Report() {}

  public static String format(String nativeSummary, String resource, String asset, String bridge) {
    return "native="
        + nativeSummary
        + "\n"
        + "resource="
        + resource
        + "\n"
        + "asset="
        + asset
        + "\n"
        + "bridge="
        + bridge;
  }
}
