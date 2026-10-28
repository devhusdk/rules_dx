package dx.quality.javac;

import java.io.IOException;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/** Self-checking test for {@link JavacCheck}; exits non-zero on the first failed expectation. */
public final class JavacCheckTest {
  private JavacCheckTest() {}

  public static void main(String[] args) throws IOException {
    Path dir = Files.createTempDirectory("javac_check_test");
    clean(dir);
    typeError(dir);
    werror(dir);
    srcjar(dir);
    sorted(dir);
    System.out.println("ok");
  }

  private static Path write(Path dir, String name, String body) throws IOException {
    Path path = dir.resolve(name);
    Files.createDirectories(path.getParent());
    Files.writeString(path, body);
    return path;
  }

  private static void expect(boolean condition, String what, String log) {
    if (!condition) {
      throw new AssertionError(what + "; log: " + log);
    }
  }

  private static void clean(Path dir) throws IOException {
    Path a = write(dir, "clean/A.java", "class A { int f() { return 1; } }\n");
    String log = JavacCheck.check(List.of("source=" + a));
    expect(log.contains("\"exit\":0"), "clean exits 0", log);
    expect(log.contains("\"diagnostics\":[]"), "clean has no diagnostics", log);
  }

  private static void typeError(Path dir) throws IOException {
    Path a = write(dir, "err/B.java", "class B {\n  int f() { return \"s\"; }\n}\n");
    String log = JavacCheck.check(List.of("source=" + a));
    expect(log.contains("\"exit\":1"), "error exits 1", log);
    expect(log.contains("\"line\":2"), "error is on line 2", log);
    expect(log.contains("\"severity\":\"error\""), "severity error", log);
    expect(log.contains("compiler.err.prob.found.req"), "error code", log);
    expect(
        log.contains(a.toString().replace("\\", "\\\\")), "file is the path javac received", log);
  }

  private static void werror(Path dir) throws IOException {
    Path a =
        write(dir, "werror/C.java", "import java.util.*;\nclass C { List l = new ArrayList(); }\n");
    String log =
        JavacCheck.check(List.of("source=" + a, "javacopt=-Xlint:all", "javacopt=-Werror"));
    expect(log.contains("\"exit\":1"), "werror exits 1", log);
    expect(log.contains("\"severity\":\"warning\""), "warning kept", log);
    expect(log.contains("compiler.err.warnings.and.werror"), "unlocated werror error", log);
  }

  private static void srcjar(Path dir) throws IOException {
    Path jar = dir.resolve("gen.srcjar");
    try (OutputStream out = Files.newOutputStream(jar);
        ZipOutputStream zip = new ZipOutputStream(out)) {
      zip.putNextEntry(new ZipEntry("p/D.java"));
      zip.write("package p;\nclass D { void f() { missing(); } }\n".getBytes());
      zip.closeEntry();
    }
    String log = JavacCheck.check(List.of("srcjar=" + jar));
    expect(log.contains("\"exit\":1"), "srcjar error exits 1", log);
    expect(log.contains("compiler.err.cant.resolve.location.args"), "srcjar error seen", log);
  }

  private static void sorted(Path dir) throws IOException {
    Path b = write(dir, "sorted/Z.java", "class Z { void f() { zz(); } }\n");
    Path a = write(dir, "sorted/A.java", "class A { void f() { aa(); } }\n");
    String log = JavacCheck.check(List.of("source=" + b, "source=" + a));
    expect(log.indexOf("sorted/A.java") < log.indexOf("sorted/Z.java"), "sorted by file", log);
    expect(
        log.equals(JavacCheck.check(List.of("source=" + a, "source=" + b))), "deterministic", log);
  }
}
