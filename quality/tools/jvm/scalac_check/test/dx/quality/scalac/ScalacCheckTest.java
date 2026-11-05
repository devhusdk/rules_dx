package dx.quality.scalac;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * Self-checking test for {@link ScalacCheck}; exits non-zero on the first failed expectation.
 *
 * <p>Each argument is the runfiles path of one compiler or library jar.
 */
public final class ScalacCheckTest {
  private static List<String> jars = new ArrayList<>();

  private ScalacCheckTest() {}

  public static void main(String[] args) throws IOException {
    String runfiles = System.getenv("TEST_SRCDIR");
    for (String arg : args) {
      for (String part : arg.split(" ")) {
        String path = Path.of(runfiles, part).toString();
        jars.add("compiler=" + path);
        if (part.contains("scala-library") || part.contains("scala_library")) {
          jars.add("classpath=" + path);
        }
      }
    }
    Path dir = Files.createTempDirectory("scalac_check_test");
    clean(dir);
    typeError(dir);
    deprecation(dir);
    werror(dir);
    unused(dir);
    unusedOverridesInfo(dir);
    srcjar(dir);
    javaSource(dir);
    malformed();
    settleDropsDepOnlyWerror();
    settleKeepsMixedWerror();
    analyzerKeyIsAccepted(dir);
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

  private static void clean(Path dir) throws IOException {
    Path a = write(dir, "clean/A.scala", "object A { def f: Int = 1 }\n");
    String log = check(List.of("source=" + a));
    expect(log.contains("\"exit\":0"), "clean exits 0", log);
    expect(log.contains("\"diagnostics\":[]"), "clean has no diagnostics", log);
  }

  private static void typeError(Path dir) throws IOException {
    // A tab and a non-BMP character (two UTF-16 units) precede the error on line 2.
    Path a =
        write(
            Path.of(""),
            "scalac_err/B.scala",
            "object B {\n\tval s = \"😀\"; val n: Int = \"s\"\n}\n");
    String log = check(List.of("source=" + a));
    expect(log.contains("\"exit\":1"), "error exits 1", log);
    expect(log.contains("\"file\":\"scalac_err/B.scala\""), "file is relative to the cwd", log);
    expect(log.contains("\"line\":2,\"column\":29"), "UTF-16 column, tab not expanded", log);
    expect(log.contains("\"severity\":\"error\""), "severity error", log);
    expect(log.contains("\"code\":\"scalac\""), "coarse code", log);
    expect(log.contains("\"tool\":\"scalac\""), "tool", log);
    expect(
        log.indexOf("\"severity\":\"error\"") == log.lastIndexOf("\"severity\":\"error\""),
        "exactly one error",
        log);
  }

  private static final String DEPRECATED =
      "object D {\n  @deprecated(\"no\", \"1\") def old: Int = 1\n  def g: Int = old\n}\n";

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
    expect(log.contains("\"severity\":\"warning\""), "warning kept", log);
    expect(log.contains("\"code\":\"" + ScalacCheck.WERROR_CODE + "\""), "werror sentinel", log);
  }

  private static final String UNUSED =
      "import scala.collection.mutable\nobject U {\n  private val n: Int = 1\n"
          + "  def f: Int = 2\n}\n";

  private static void unused(Path dir) throws IOException {
    Path a = write(dir, "unused/U.scala", UNUSED);
    String log = check(List.of("source=" + a, "scalacopt=-Wunused:_"));
    expect(log.contains("\"exit\":0"), "unused code does not fail", log);
    String warning = "\"severity\":\"warning\"";
    expect(occurrences(log, warning) == 2, "one import and one private val", log);
    expect(log.contains("Unused import"), "unused import reported", log);
    expect(log.contains("is never used"), "unused private val reported", log);
  }

  private static void unusedOverridesInfo(Path dir) throws IOException {
    Path a = write(dir, "unused_info/U.scala", UNUSED);
    String log =
        check(
            List.of(
                "source=" + a,
                "scalacopt=-Wunused:_",
                "scalacopt=-Wconf:cat=unused:info",
                "scalacopt=-Wconf:cat=unused:w"));
    expect(occurrences(log, "\"severity\":\"warning\"") == 2, "last -Wconf keeps warnings", log);
  }

  private static int occurrences(String text, String what) {
    return text.split(Pattern.quote(what), -1).length - 1;
  }

  private static void srcjar(Path dir) throws IOException {
    Path jar = dir.resolve("gen.srcjar");
    try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(jar))) {
      zip.putNextEntry(new ZipEntry("pkg/G.scala"));
      zip.write(
          "object G { val n: Int = \"s\" }\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));
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

  private static ScalacCheck.Entry entry(String severity, String code) {
    return new ScalacCheck.Entry("", 0, 0, severity, code, "m");
  }

  private static void settleDropsDepOnlyWerror() {
    List<ScalacCheck.Entry> entries = new ArrayList<>();
    entries.add(entry("warning", ScalacCheck.UNUSED_DEP_CODE));
    entries.add(entry("error", ScalacCheck.WERROR_CODE));
    expect(ScalacCheck.settle(entries) == 0, "dep-only warnings exit 0", "");
    expect(entries.size() == 1, "werror entry is dropped", "");
  }

  private static void settleKeepsMixedWerror() {
    List<ScalacCheck.Entry> entries = new ArrayList<>();
    entries.add(entry("warning", ScalacCheck.UNUSED_DEP_CODE));
    entries.add(entry("warning", ScalacCheck.GENERIC_CODE));
    entries.add(entry("error", ScalacCheck.WERROR_CODE));
    expect(ScalacCheck.settle(entries) == 1, "mixed warnings still fail", "");
    expect(entries.size() == 3, "werror entry is kept", "");
  }

  private static void analyzerKeyIsAccepted(Path dir) throws IOException {
    Path a = write(dir, "analyzer/A.scala", "object A { def f: Int = 1 }\n");
    try {
      check(List.of("source=" + a, "analyzer=" + dir.resolve("missing.jar")));
    } catch (IOException err) {
      expect(
          !String.valueOf(err.getMessage()).contains("unknown request key"),
          "analyzer= is a known key",
          err.getMessage());
    }
  }
}
