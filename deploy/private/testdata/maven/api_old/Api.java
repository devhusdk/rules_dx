package com.example.api;

/** The previous release of the API fixture. */
public final class Api {
  private Api() {}

  /** Stays in every release. */
  public static String keep() {
    return "keep";
  }

  /** Removed in the next release. */
  public static String gone() {
    return "gone";
  }
}
