package dx.quality.javac;

import java.io.IOException;
import java.io.InputStream;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.StandardLocation;
import javax.tools.ToolProvider;

/** Runs javac up to flow analysis on a target's sources and renders a versioned JSON log. */
public final class JavacCheck {
  private static final int VERSION = 1;

  private JavacCheck() {}

  /** One javac diagnostic as written to the log. */
  static final class Entry {
    private final String file;
    private final long line;
    private final long column;
    private final String severity;
    private final String code;
    private final String message;

    Entry(String file, long line, long column, String severity, String code, String message) {
      this.file = file;
      this.line = line;
      this.column = column;
      this.severity = severity;
      this.code = code;
      this.message = message;
    }

    String file() {
      return file;
    }

    long line() {
      return line;
    }

    long column() {
      return column;
    }

    String severity() {
      return severity;
    }

    String code() {
      return code;
    }

    String message() {
      return message;
    }
  }

  /**
   * Checks the request and returns the JSON log.
   *
   * <p>Each request line is {@code key=value}. Keys are {@code source}, {@code srcjar}, {@code
   * classpath}, {@code bootclasspath}, {@code processorpath}, {@code processor} and {@code
   * javacopt}; they may repeat. Without a {@code processor} the check runs with {@code -proc:none}.
   */
  public static String check(List<String> request) throws IOException {
    List<String> sources = new ArrayList<>();
    List<String> srcjars = new ArrayList<>();
    List<String> classpath = new ArrayList<>();
    List<String> bootclasspath = new ArrayList<>();
    List<String> processorpath = new ArrayList<>();
    List<String> processors = new ArrayList<>();
    List<String> options = new ArrayList<>();
    for (String line : request) {
      if (line.isEmpty()) {
        continue;
      }
      int split = line.indexOf('=');
      if (split < 0) {
        throw new IOException("malformed request line: " + line);
      }
      String value = line.substring(split + 1);
      switch (line.substring(0, split)) {
        case "source":
          sources.add(value);
          break;
        case "srcjar":
          srcjars.add(value);
          break;
        case "classpath":
          classpath.add(value);
          break;
        case "bootclasspath":
          bootclasspath.add(value);
          break;
        case "processorpath":
          processorpath.add(value);
          break;
        case "processor":
          processors.add(value);
          break;
        case "javacopt":
          options.add(value);
          break;
        default:
          throw new IOException("unknown request key: " + line.substring(0, split));
      }
    }
    JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
    if (compiler == null) {
      throw new IOException("the running JVM has no system java compiler");
    }
    Path work = Files.createTempDirectory("javac_check");
    try {
      Path classes = Files.createDirectory(work.resolve("classes"));
      List<String> inputs = new ArrayList<>(sources);
      java.util.Map<String, String> names = new java.util.LinkedHashMap<>();
      int index = 0;
      for (String srcjar : srcjars) {
        inputs.addAll(
            extract(srcjar, Files.createDirectory(work.resolve("srcjar" + index++)), names));
      }
      List<String> args = new ArrayList<>(options);
      args.addAll(
          List.of(
              "-XDshould-stop.ifError=FLOW",
              "-XDshould-stop.ifNoError=FLOW",
              "-d",
              classes.toString()));
      if (!classpath.isEmpty()) {
        args.addAll(List.of("-classpath", String.join(java.io.File.pathSeparator, classpath)));
      }
      if (!bootclasspath.isEmpty()) {
        // A platform class path stands in for --system, which javac warns about under -source.
        args.add("-Xlint:-options");
      }
      if (processors.isEmpty()) {
        args.add("-proc:none");
      } else {
        args.addAll(List.of("-processor", String.join(",", processors)));
        if (!processorpath.isEmpty()) {
          args.addAll(
              List.of("-processorpath", String.join(java.io.File.pathSeparator, processorpath)));
        }
      }
      DiagnosticCollector<JavaFileObject> collector = new DiagnosticCollector<>();
      int exit;
      try (StandardJavaFileManager files =
          compiler.getStandardFileManager(collector, Locale.ROOT, StandardCharsets.UTF_8)) {
        if (!bootclasspath.isEmpty()) {
          // The -bootclasspath flag is rejected with -source/-target 9+; JavaBuilder sets the
          // location on the file manager instead.
          List<Path> boot = new ArrayList<>();
          for (String entry : bootclasspath) {
            boot.add(Path.of(entry));
          }
          files.setLocationFromPaths(StandardLocation.PLATFORM_CLASS_PATH, boot);
        }
        List<java.io.File> units = new ArrayList<>();
        for (String input : inputs) {
          units.add(new java.io.File(input));
        }
        boolean ok =
            compiler
                .getTask(
                    new StringWriter(),
                    files,
                    collector,
                    args,
                    null,
                    files.getJavaFileObjectsFromFiles(units))
                .call();
        exit = ok ? 0 : 1;
      }
      List<Entry> entries = new ArrayList<>();
      for (Diagnostic<? extends JavaFileObject> d : collector.getDiagnostics()) {
        entries.add(entry(d, names));
      }
      entries.sort(
          Comparator.comparing(Entry::file)
              .thenComparingLong(Entry::line)
              .thenComparingLong(Entry::column)
              .thenComparing(Entry::code)
              .thenComparing(Entry::message));
      return render(exit, entries);
    } finally {
      delete(work);
    }
  }

  private static Entry entry(
      Diagnostic<? extends JavaFileObject> d, java.util.Map<String, String> names) {
    String severity;
    switch (d.getKind()) {
      case ERROR:
        severity = "error";
        break;
      case WARNING:
      case MANDATORY_WARNING:
        severity = "warning";
        break;
      default:
        severity = "note";
        break;
    }
    JavaFileObject source = d.getSource();
    boolean located = source != null && d.getLineNumber() > 0 && d.getColumnNumber() > 0;
    return new Entry(
        source == null ? "" : names.getOrDefault(source.getName(), source.getName()),
        located ? d.getLineNumber() : 0,
        located ? d.getColumnNumber() : 0,
        severity,
        String.valueOf(d.getCode()),
        stable(d.getMessage(Locale.ROOT), names));
  }

  /** Replaces temp extraction paths with their stable srcjar names. */
  private static String stable(String message, java.util.Map<String, String> names) {
    String result = message;
    for (java.util.Map.Entry<String, String> name : names.entrySet()) {
      result = result.replace(name.getKey(), name.getValue());
    }
    return result;
  }

  static String render(int exit, List<Entry> entries) {
    StringBuilder out = new StringBuilder();
    out.append("{\"version\":").append(VERSION);
    out.append(",\"tool\":\"javac\",\"exit\":").append(exit);
    out.append(",\"diagnostics\":[");
    boolean first = true;
    for (Entry e : entries) {
      if (!first) {
        out.append(',');
      }
      first = false;
      out.append("{\"file\":").append(quote(e.file()));
      out.append(",\"line\":").append(e.line());
      out.append(",\"column\":").append(e.column());
      out.append(",\"severity\":").append(quote(e.severity()));
      out.append(",\"code\":").append(quote(e.code()));
      out.append(",\"message\":").append(quote(e.message())).append('}');
    }
    return out.append("]}\n").toString();
  }

  private static String quote(String text) {
    StringBuilder out = new StringBuilder("\"");
    for (int i = 0; i < text.length(); i++) {
      char c = text.charAt(i);
      if (c == '"') {
        out.append("\\\"");
      } else if (c == '\\') {
        out.append("\\\\");
      } else if (c == '\n') {
        out.append("\\n");
      } else if (c == '\r') {
        out.append("\\r");
      } else if (c == '\t') {
        out.append("\\t");
      } else if (c < 0x20) {
        out.append(String.format("\\u%04x", (int) c));
      } else {
        out.append(c);
      }
    }
    return out.append('"').toString();
  }

  private static List<String> extract(
      String srcjar, Path target, java.util.Map<String, String> names) throws IOException {
    List<String> extracted = new ArrayList<>();
    try (ZipFile zip = new ZipFile(srcjar)) {
      var entries = zip.entries();
      while (entries.hasMoreElements()) {
        ZipEntry entry = entries.nextElement();
        if (entry.isDirectory() || !entry.getName().endsWith(".java")) {
          continue;
        }
        Path out = target.resolve(entry.getName()).normalize();
        if (!out.startsWith(target)) {
          throw new IOException("source jar entry escapes its directory: " + entry.getName());
        }
        Files.createDirectories(out.getParent());
        try (InputStream in = zip.getInputStream(entry)) {
          Files.copy(in, out, StandardCopyOption.REPLACE_EXISTING);
        }
        extracted.add(out.toString());
        names.put(out.toString(), srcjar + "!/" + entry.getName());
      }
    }
    return extracted;
  }

  private static void delete(Path root) throws IOException {
    try (Stream<Path> walk = Files.walk(root)) {
      for (Path path : (Iterable<Path>) walk.sorted(Comparator.reverseOrder())::iterator) {
        Files.delete(path);
      }
    }
  }
}
