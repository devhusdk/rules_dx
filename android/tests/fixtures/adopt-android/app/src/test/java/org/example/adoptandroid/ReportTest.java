package org.example.adoptandroid;

public final class ReportTest {
  public static void main(String[] args) {
    String got = Report.format("words=2", "Adopt Android", "hello world", "jni");
    String want = "native=words=2\nresource=Adopt Android\nasset=hello world\nbridge=jni";
    if (!got.equals(want)) {
      System.err.println("got '" + got + "', want '" + want + "'");
      System.exit(1);
    }
  }
}
