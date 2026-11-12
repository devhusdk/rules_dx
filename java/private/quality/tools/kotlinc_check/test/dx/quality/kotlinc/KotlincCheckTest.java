package dx.quality.kotlinc;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Self-checking test for {@link KotlincCheck}; exits non-zero on the first failed expectation.
 *
 * <p>Each argument is the runfiles path of one compiler jar.
 */
public final class KotlincCheckTest {
  private static List<String> compiler = new ArrayList<>();
  private static String stdlib = "";

  private KotlincCheckTest() {}

  public static void main(String[] args) throws IOException {
    String runfiles = System.getenv("TEST_SRCDIR");
    for (String arg : args) {
      for (String part : arg.split(" ")) {
        String path = Path.of(runfiles, part).toString();
        compiler.add("compiler=" + path);
        if (part.endsWith("kotlin-stdlib.jar")) {
          stdlib = "classpath=" + path;
        }
      }
    }
    Path dir = Files.createTempDirectory("kotlinc_check_test");
    clean(dir);
    typeError(dir);
    werror(dir);
    javaSource(dir);
    friend(dir);
    System.out.println("ok");
  }

  private static String check(List<String> request) throws IOException {
    List<String> all = new ArrayList<>(request);
    all.addAll(compiler);
    all.add(stdlib);
    return KotlincCheck.check(all);
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
    Path a = write(dir, "clean/A.kt", "fun f(): Int = 1\n");
    String log = check(List.of("source=" + a));
    expect(log.contains("\"exit\":0"), "clean exits 0", log);
    expect(log.contains("\"diagnostics\":[]"), "clean has no diagnostics", log);
  }

  private static void typeError(Path dir) throws IOException {
    Path a = write(Path.of(""), "kotlinc_err/B.kt", "fun f(): Int {\n\treturn \"s\"\n}\n");
    String log = check(List.of("source=" + a));
    expect(log.contains("\"exit\":1"), "error exits 1", log);
    expect(log.contains("\"file\":\"kotlinc_err/B.kt\""), "file is relative to the cwd", log);
    expect(log.contains("\"line\":2,\"column\":9"), "UTF-16 column, tab not expanded", log);
    expect(log.contains("\"severity\":\"error\""), "severity error", log);
    expect(log.contains("\"code\":\"RETURN_TYPE_MISMATCH\""), "internal diagnostic name", log);
    expect(
        log.indexOf("\"severity\":\"error\"") == log.lastIndexOf("\"severity\":\"error\""),
        "exactly one error",
        log);
  }

  private static void werror(Path dir) throws IOException {
    Path a = write(dir, "werror/C.kt", "fun f(): Int { val a = 1 as Int; return a }\n");
    String log = check(List.of("source=" + a, "kotlincopt=-Werror"));
    expect(log.contains("\"exit\":1"), "werror exits 1", log);
    expect(log.contains("\"severity\":\"warning\""), "warning kept", log);
    expect(log.contains("\"code\":\"" + KotlincCheck.WERROR_CODE + "\""), "werror sentinel", log);
  }

  private static void javaSource(Path dir) throws IOException {
    Path j =
        write(dir, "mixed/J.java", "public class J { public static int one() { return 1; } }\n");
    Path k = write(dir, "mixed/K.kt", "fun f(): Int = J.one()\n");
    String log = check(List.of("source=" + k, "source=" + j));
    expect(log.contains("\"exit\":0"), "java symbols resolve without a false error", log);
  }

  private static void friend(Path dir) throws IOException {
    // A friend path is a class directory or jar. Without one, the internal member is an error.
    Path a = write(dir, "friend/A.kt", "fun f(): Int = 1\n");
    String log = check(List.of("source=" + a, "module=lib", "friend=" + dir.resolve("none.jar")));
    expect(log.contains("\"exit\":0"), "module and friend flags are accepted", log);
  }
}
