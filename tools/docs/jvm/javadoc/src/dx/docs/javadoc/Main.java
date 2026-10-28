package dx.docs.javadoc;

import java.io.IOException;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.DocumentationTool;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;

/** Runs the Javadoc tool in-process with {@link IrDoclet} and writes one API record file. */
public final class Main {
  private Main() {}

  /** Entry point; exits 1 and writes no output when the sources do not compile. */
  public static void main(String[] args) {
    try {
      System.exit(run(args));
    } catch (IOException error) {
      System.err.println("javadoc_extract: " + error.getMessage());
      System.exit(1);
    }
  }

  private static int run(String[] args) throws IOException {
    String manifest = null;
    String classpathFile = null;
    String release = null;
    String out = null;
    for (int index = 0; index < args.length; index += 2) {
      if (index + 1 >= args.length) {
        return usage(args[index] + " needs a value");
      }
      String value = args[index + 1];
      switch (args[index]) {
        case "--srcs-manifest":
          manifest = value;
          break;
        case "--classpath-file":
          classpathFile = value;
          break;
        case "--release":
          release = value;
          break;
        case "--out":
          out = value;
          break;
        default:
          return usage("unknown argument '" + args[index] + "'");
      }
    }
    if (manifest == null || classpathFile == null || release == null || out == null) {
      return usage("--srcs-manifest, --classpath-file, --release and --out are required");
    }
    DocumentationTool tool = ToolProvider.getSystemDocumentationTool();
    if (tool == null) {
      System.err.println(
          "javadoc_extract: the Java runtime has no jdk.javadoc module; use a full JDK");
      return 1;
    }
    List<Path> sources = new ArrayList<>();
    for (String line : Files.readAllLines(Paths.get(manifest), StandardCharsets.UTF_8)) {
      if (line.isEmpty()) {
        continue;
      }
      Path source = Paths.get(line.split("\t", -1)[0]);
      if (!Files.isRegularFile(source)) {
        System.err.println("javadoc_extract: missing source " + source);
        return 1;
      }
      sources.add(source);
    }
    if (sources.isEmpty()) {
      System.err.println("javadoc_extract: the srcs manifest lists no sources");
      return 1;
    }
    List<String> classpath = new ArrayList<>();
    for (String line : Files.readAllLines(Paths.get(classpathFile), StandardCharsets.UTF_8)) {
      if (line.isEmpty()) {
        continue;
      }
      if (!Files.exists(Paths.get(line))) {
        System.err.println("javadoc_extract: missing classpath entry " + line);
        return 1;
      }
      classpath.add(line);
    }
    Path output = Paths.get(out);
    Files.deleteIfExists(output);
    DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
    boolean ok;
    try (StandardJavaFileManager files =
        tool.getStandardFileManager(diagnostics, null, StandardCharsets.UTF_8)) {
      List<String> options = new ArrayList<>();
      options.add("-public");
      options.add("--release");
      options.add(release);
      options.add("-classpath");
      options.add(String.join(java.io.File.pathSeparator, classpath));
      options.add("-encoding");
      options.add("UTF-8");
      options.add("-quiet");
      options.add("--dx-out");
      options.add(out);
      options.add("--dx-manifest");
      options.add(manifest);
      StringWriter log = new StringWriter();
      DocumentationTool.DocumentationTask task =
          tool.getTask(
              new PrintWriter(log),
              files,
              diagnostics,
              IrDoclet.class,
              options,
              files.getJavaFileObjectsFromPaths(sources));
      ok = task.call();
      if (!log.toString().isEmpty()) {
        System.err.print(log);
      }
    }
    boolean errors = false;
    for (Diagnostic<? extends JavaFileObject> diagnostic : diagnostics.getDiagnostics()) {
      System.err.println(diagnostic);
      errors |= diagnostic.getKind() == Diagnostic.Kind.ERROR;
    }
    if (!ok || errors || !Files.isRegularFile(output)) {
      Files.deleteIfExists(output);
      System.err.println("javadoc_extract: the sources could not be documented");
      return 1;
    }
    return 0;
  }

  private static int usage(String message) {
    System.err.println(
        "javadoc_extract: "
            + message
            + "\nusage: javadoc_extract --srcs-manifest FILE --classpath-file FILE"
            + " --release N --out FILE");
    return 2;
  }
}
