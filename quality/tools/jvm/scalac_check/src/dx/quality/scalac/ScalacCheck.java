package dx.quality.scalac;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * Runs scalac on a target's sources without keeping classes and renders a versioned JSON log.
 *
 * <p>The driver is never built against scala-compiler. It loads the consumer toolchain's own
 * compiler jars in an isolated class loader and talks to {@code Global} and {@code StoreReporter}
 * by reflection, so it follows whichever Scala 2.13 version the consumer uses.
 */
public final class ScalacCheck {
  private static final int VERSION = 1;

  /**
   * Code of the error reported when {@code -Werror} turned warnings into a failure. scalac reports
   * that error without a position or a name, and the runner's parser looks for this exact code.
   */
  static final String WERROR_CODE = "scalac.warnings.as.errors";

  /** Code of every scalac diagnostic: Scala 2 has no diagnostic ids. */
  static final String GENERIC_CODE = "scalac";

  private static final long STACK_BYTES = 64L * 1024 * 1024;

  private ScalacCheck() {}

  /** One scalac diagnostic as written to the log. */
  static final class Entry {
    final String file;
    final long line;
    final long column;
    final String severity;
    final String code;
    final String message;

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
   * <p>Each request line is {@code key=value}. Keys are {@code source} (.scala, or .java only to
   * resolve symbols), {@code srcjar}, {@code classpath}, {@code compiler} (the toolchain's compiler
   * jars), {@code plugin} and {@code scalacopt}; they may repeat.
   */
  public static String check(List<String> request) throws IOException {
    List<String> sources = new ArrayList<>();
    List<String> srcjars = new ArrayList<>();
    List<String> classpath = new ArrayList<>();
    List<String> compiler = new ArrayList<>();
    List<String> plugins = new ArrayList<>();
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
        case "compiler":
          compiler.add(value);
          break;
        case "plugin":
          plugins.add(value);
          break;
        case "scalacopt":
          options.add(value);
          break;
        default:
          throw new IOException("unknown request key: " + line.substring(0, split));
      }
    }
    if (compiler.isEmpty()) {
      throw new IOException("request has no compiler= jars");
    }
    Path work = Files.createTempDirectory("scalac_check");
    try {
      Path classes = Files.createDirectory(work.resolve("classes"));
      List<String> inputs = new ArrayList<>(sources);
      Map<String, String> names = new LinkedHashMap<>();
      int index = 0;
      for (String srcjar : srcjars) {
        inputs.addAll(
            extract(srcjar, Files.createDirectory(work.resolve("srcjar" + index++)), names));
      }
      List<String> args = new ArrayList<>(options);
      args.addAll(List.of("-d", classes.toString()));
      args.addAll(List.of("-classpath", String.join(File.pathSeparator, classpath)));
      for (String plugin : plugins) {
        args.add("-Xplugin:" + plugin);
      }
      List<Entry> entries = new ArrayList<>();
      int exit = runOnBigStack(compiler, args, inputs, entries, names);
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

  /** scalac recurses deeply, so it runs on a thread with a large stack. */
  private static int runOnBigStack(
      List<String> compilerJars,
      List<String> args,
      List<String> inputs,
      List<Entry> entries,
      Map<String, String> names)
      throws IOException {
    Object[] outcome = new Object[2];
    Thread thread =
        new Thread(
            null,
            () -> {
              try {
                outcome[0] = run(compilerJars, args, inputs, entries, names);
              } catch (IOException | RuntimeException | StackOverflowError err) {
                outcome[1] = err;
              }
            },
            "scalac_check",
            STACK_BYTES);
    thread.start();
    try {
      thread.join();
    } catch (InterruptedException err) {
      Thread.currentThread().interrupt();
      throw new IOException("interrupted", err);
    }
    if (outcome[1] instanceof IOException) {
      throw (IOException) outcome[1];
    }
    if (outcome[1] != null) {
      throw new IOException("scalac_check failed: " + outcome[1], (Throwable) outcome[1]);
    }
    return (Integer) outcome[0];
  }

  private static int run(
      List<String> compilerJars,
      List<String> args,
      List<String> inputs,
      List<Entry> entries,
      Map<String, String> names)
      throws IOException {
    URL[] urls = new URL[compilerJars.size()];
    for (int i = 0; i < urls.length; i++) {
      urls[i] = new File(compilerJars.get(i)).toURI().toURL();
    }
    Thread thread = Thread.currentThread();
    ClassLoader previous = thread.getContextClassLoader();
    try (URLClassLoader loader = new URLClassLoader(urls, ClassLoader.getPlatformClassLoader())) {
      thread.setContextClassLoader(loader);
      // Only the Scala 2 frontend exists today; a Scala 3 one would be chosen here by class.
      Class<?> settingsClass = loader.loadClass("scala.tools.nsc.Settings");
      Class<?> reporterClass = loader.loadClass("scala.tools.nsc.reporters.Reporter");
      Class<?> storeClass = loader.loadClass("scala.tools.nsc.reporters.StoreReporter");
      Class<?> globalClass = loader.loadClass("scala.tools.nsc.Global");
      Class<?> runClass = loader.loadClass("scala.tools.nsc.Global$Run");
      Class<?> onceClass = loader.loadClass("scala.collection.IterableOnce");
      Class<?> listClass = loader.loadClass("scala.collection.immutable.List$");
      Class<?> converters = loader.loadClass("scala.jdk.javaapi.CollectionConverters");
      Object listModule = listClass.getField("MODULE$").get(null);
      Method from = listClass.getMethod("from", onceClass);
      Method asScala = converters.getMethod("asScala", Iterable.class);

      Object settings = settingsClass.getConstructor().newInstance();
      Object parsed =
          settingsClass
              .getMethod("processArguments", from.getReturnType(), boolean.class)
              .invoke(settings, from.invoke(listModule, asScala.invoke(null, args)), true);
      Object ok = parsed.getClass().getMethod("_1").invoke(parsed);
      if (!Boolean.TRUE.equals(ok)) {
        throw new IOException("scalac rejected its options: " + args);
      }
      Object reporter = storeClass.getConstructor(settingsClass).newInstance(settings);
      Object global =
          globalClass.getConstructor(settingsClass, reporterClass).newInstance(settings, reporter);
      Object compileRun = runClass.getConstructor(globalClass).newInstance(global);
      runClass
          .getMethod("compile", from.getReturnType())
          .invoke(compileRun, from.invoke(listModule, asScala.invoke(null, inputs)));
      reporterClass.getMethod("finish").invoke(reporter);
      boolean errors = (Boolean) reporterClass.getMethod("hasErrors").invoke(reporter);
      collect(storeClass.getMethod("infos").invoke(reporter), entries, names);
      return errors ? 1 : 0;
    } catch (InvocationTargetException err) {
      throw new IOException("scalac failed: " + err.getCause(), err.getCause());
    } catch (ReflectiveOperationException err) {
      throw new IOException("the compiler jars do not look like scalac 2.13: " + err, err);
    } finally {
      thread.setContextClassLoader(previous);
    }
  }

  private static void collect(Object infos, List<Entry> entries, Map<String, String> names)
      throws ReflectiveOperationException {
    Object it = infos.getClass().getMethod("iterator").invoke(infos);
    Method hasNext = it.getClass().getMethod("hasNext");
    Method next = it.getClass().getMethod("next");
    while ((Boolean) hasNext.invoke(it)) {
      record(next.invoke(it), entries, names);
    }
  }

  private static void record(Object info, List<Entry> entries, Map<String, String> names)
      throws ReflectiveOperationException {
    Class<?> type = info.getClass();
    String severity;
    switch (String.valueOf(type.getMethod("severity").invoke(info))) {
      case "ERROR":
        severity = "error";
        break;
      case "WARNING":
        severity = "warning";
        break;
      case "INFO":
        severity = "note";
        break;
      default:
        return;
    }
    String message = String.valueOf(type.getMethod("msg").invoke(info));
    String file = "";
    long line = 0;
    long column = 0;
    Object pos = type.getMethod("pos").invoke(info);
    if ((Boolean) pos.getClass().getMethod("isDefined").invoke(pos)) {
      Object source = pos.getClass().getMethod("source").invoke(pos);
      String path = String.valueOf(source.getClass().getMethod("path").invoke(source));
      int l = ((Number) pos.getClass().getMethod("line").invoke(pos)).intValue();
      int point = ((Number) pos.getClass().getMethod("point").invoke(pos)).intValue();
      // Scalac's own column expands tabs; count UTF-16 units from the start of the line instead.
      int start =
          ((Number) source.getClass().getMethod("lineToOffset", int.class).invoke(source, l - 1))
              .intValue();
      if (l > 0 && point >= start) {
        file = names.getOrDefault(path, relative(path));
        line = l;
        column = point - start + 1;
      }
    }
    String code = GENERIC_CODE;
    if (severity.equals("error") && file.isEmpty() && message.contains("-Werror")) {
      code = WERROR_CODE;
    }
    entries.add(new Entry(file, line, column, severity, code, stable(message, names)));
  }

  /**
   * scalac reports the paths it was given; the runner matches the working-directory-relative ones.
   */
  private static String relative(String path) {
    String cwd = Path.of("").toAbsolutePath().toString() + File.separator;
    return path.startsWith(cwd) ? path.substring(cwd.length()) : path;
  }

  /** Replaces temp extraction paths with their stable srcjar names. */
  private static String stable(String message, Map<String, String> names) {
    String result = message;
    for (Map.Entry<String, String> name : names.entrySet()) {
      result = result.replace(name.getKey(), name.getValue());
    }
    return result;
  }

  static String render(int exit, List<Entry> entries) {
    StringBuilder out = new StringBuilder();
    out.append("{\"version\":").append(VERSION);
    out.append(",\"tool\":\"scalac\",\"exit\":").append(exit);
    out.append(",\"diagnostics\":[");
    boolean first = true;
    for (Entry e : entries) {
      if (!first) {
        out.append(',');
      }
      first = false;
      out.append("{\"file\":").append(quote(e.file));
      out.append(",\"line\":").append(e.line);
      out.append(",\"column\":").append(e.column);
      out.append(",\"severity\":").append(quote(e.severity));
      out.append(",\"code\":").append(quote(e.code));
      out.append(",\"message\":").append(quote(e.message)).append('}');
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

  private static List<String> extract(String srcjar, Path target, Map<String, String> names)
      throws IOException {
    List<String> extracted = new ArrayList<>();
    try (ZipFile zip = new ZipFile(srcjar)) {
      var entries = zip.entries();
      while (entries.hasMoreElements()) {
        ZipEntry entry = entries.nextElement();
        String name = entry.getName();
        if (entry.isDirectory() || !(name.endsWith(".scala") || name.endsWith(".java"))) {
          continue;
        }
        Path out = target.resolve(name).normalize();
        if (!out.startsWith(target)) {
          throw new IOException("source jar entry escapes its directory: " + name);
        }
        Files.createDirectories(out.getParent());
        try (InputStream in = zip.getInputStream(entry)) {
          Files.copy(in, out, StandardCopyOption.REPLACE_EXISTING);
        }
        extracted.add(out.toString());
        names.put(out.toString(), srcjar + "!/" + name);
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
