package dx.quality.scalac;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * Self-checking test for the Scala 3 frontend of {@link ScalacCheck}; exits non-zero on the first
 * failed expectation.
 *
 * <p>Each argument is the runfiles path of one Scala 3 compiler jar.
 */
public final class Scala3CheckTest {
  private static List<String> jars = new ArrayList<>();

  private Scala3CheckTest() {}

  public static void main(String[] args) throws IOException {
    String runfiles = System.getenv("TEST_SRCDIR");
    for (String arg : args) {
      for (String part : arg.split(" ")) {
        String path = Path.of(runfiles, part).toString();
        jars.add("compiler=" + path);
        if (part.contains("scala-library") || part.contains("scala3-library")) {
          jars.add("classpath=" + path);
        }
      }
    }
    Path dir = Files.createTempDirectory("scala3_check_test");
    clean(dir);
    typeError(dir);
    deprecation(dir);
    werror(dir);
    unused(dir);
    badOption(dir);
    srcjar(dir);
    javaSource(dir);
    malformed();
    System.out.println("ok");
  }

  private static String check(List<String> request) throws IOException {
    List<String> all = new ArrayList<>(request);
    all.addAll(jars);
    return ScalacCheck.check(all);
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

  private static int occurrences(String text, String what) {
    return text.split(Pattern.quote(what), -1).length - 1;
  }

  private static void clean(Path dir) throws IOException {
    Path a = write(dir, "clean/A.scala", "object A:\n  def f: Int = 1\n");
    String log = check(List.of("source=" + a));
    expect(log.contains("\"exit\":0"), "clean exits 0", log);
    expect(log.contains("\"diagnostics\":[]"), "clean has no diagnostics", log);
  }

  private static void typeError(Path dir) throws IOException {
    // A tab and a non-BMP character (two UTF-16 units) precede the error on line 2.
    Path a =
        write(
            Path.of(""),
            "scala3_err/B.scala",
            "object B {\n\tval s = \"😀\"; val n: Int = \"s\"\n}\n");
    String log = check(List.of("source=" + a));
    expect(log.contains("\"exit\":1"), "error exits 1", log);
    expect(log.contains("\"file\":\"scala3_err/B.scala\""), "file is relative to the cwd", log);
    expect(log.contains("\"line\":2,\"column\":"), "one-based line", log);
    expect(log.contains("\"column\":29"), "UTF-16 column, tab not expanded", log);
    expect(log.contains("\"tool\":\"scalac\""), "tool", log);
    expect(occurrences(log, "\"severity\":\"error\"") == 1, "exactly one error", log);
  }

  private static final String DEPRECATED =
      "object D:\n  @deprecated(\"no\", \"1\") def old: Int = 1\n  def g: Int = old\n";

  private static void deprecation(Path dir) throws IOException {
    Path a = write(dir, "deprecation/D.scala", DEPRECATED);
    String log = check(List.of("source=" + a, "scalacopt=-deprecation"));
    expect(log.contains("\"exit\":0"), "warnings do not fail", log);
    expect(log.contains("\"severity\":\"warning\""), "deprecation warning", log);
    expect(log.contains("\"line\":3"), "warning line", log);
  }

  private static void werror(Path dir) throws IOException {
    Path a = write(dir, "werror/D.scala", DEPRECATED);
    String log = check(List.of("source=" + a, "scalacopt=-deprecation", "scalacopt=-Werror"));
    expect(log.contains("\"exit\":1"), "werror exits 1", log);
    // dotc reports each warning as an error at its own position, with no sentinel entry.
    expect(log.contains("\"line\":3"), "the warning becomes an error at its line", log);
    expect(log.contains("\"severity\":\"error\""), "werror makes the warning an error", log);
  }

  private static void unused(Path dir) throws IOException {
    Path a =
        write(
            dir,
            "unused/U.scala",
            "import scala.collection.mutable\nobject U:\n  private val n: Int = 1\n"
                + "  def f: Int = 2\n");
    String log = check(List.of("source=" + a, "scalacopt=-Wunused:all"));
    expect(log.contains("\"exit\":0"), "unused code does not fail", log);
    expect(occurrences(log, "\"severity\":\"warning\"") == 2, "one import and one private val", log);
    expect(occurrences(log.toLowerCase(), "unused") >= 2, "both warnings mention unused", log);
  }

  private static void badOption(Path dir) throws IOException {
    Path a = write(dir, "badopt/A.scala", "object A:\n  def f: Int = 1\n");
    String log = check(List.of("source=" + a, "scalacopt=-source:nonsense"));
    expect(log.contains("\"exit\":1"), "a bad option exits 1", log);
    expect(log.contains("\"severity\":\"error\""), "a bad option is an error diagnostic", log);
  }

  private static void srcjar(Path dir) throws IOException {
    Path jar = dir.resolve("gen.srcjar");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(jar))) {
      zip.putNextEntry(new ZipEntry("pkg/G.scala"));
      zip.write("object G { val n: Int = \"s\" }\n".getBytes(StandardCharsets.UTF_8));
      zip.closeEntry();
    }
    String log = check(List.of("srcjar=" + jar));
    expect(log.contains("\"exit\":1"), "srcjar error exits 1", log);
    expect(log.contains("gen.srcjar!/pkg/G.scala"), "srcjar entry named stably", log);
  }

  private static void javaSource(Path dir) throws IOException {
    Path j =
        write(dir, "mixed/J.java", "public class J { public static int one() { return 1; } }\n");
    Path s = write(dir, "mixed/S.scala", "object S { def f: Int = J.one() }\n");
    String log = check(List.of("source=" + s, "source=" + j));
    expect(log.contains("\"exit\":0"), "java symbols resolve without a false error", log);
  }

  private static void malformed() {
    try {
      check(List.of("nonsense"));
    } catch (IOException err) {
      return;
    }
    throw new AssertionError("a malformed request line must fail");
  }
}
