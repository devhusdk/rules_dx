package com.example.api;

/** The new release of the API fixture, without gone(). */
public final class Api {
  private Api() {}

  /** Stays in every release. */
  public static String keep() {
    return "keep";
  }
}
