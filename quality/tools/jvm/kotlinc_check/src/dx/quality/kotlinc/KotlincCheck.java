package dx.quality.kotlinc;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
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
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * Runs kotlinc on a target's sources without keeping classes and renders a versioned JSON log.
 *
 * <p>The driver is never built against kotlin-compiler. It loads the consumer toolchain's own
 * compiler jars in an isolated class loader and talks to {@code K2JVMCompiler} and {@code
 * MessageCollector} by reflection, so it follows whichever kotlinc version the consumer uses.
 */
public final class KotlincCheck {
  private static final int VERSION = 1;

  /**
   * Code of the error reported when {@code -Werror} turned warnings into a failure. kotlinc reports
   * that error without a position or a name, and the runner's parser looks for this exact code.
   */
  static final String WERROR_CODE = "kotlinc.warnings.as.errors";

  /** Code of a diagnostic kotlinc gave no internal name. */
  static final String GENERIC_CODE = "kotlinc";

  private static final Pattern NAMED =
      Pattern.compile("^\\[([A-Z][A-Z0-9_]*)\\] (.*)$", Pattern.DOTALL);

  private KotlincCheck() {}

  /** One kotlinc diagnostic as written to the log. */
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
   * <p>Each request line is {@code key=value}. Keys are {@code source} (.kt, .kts or .java, the
   * latter only to resolve symbols), {@code srcjar}, {@code classpath}, {@code compiler} (the
   * toolchain's compiler jars), {@code plugin}, {@code pluginopt}, {@code friend}, {@code
   * kotlincopt} and {@code module}; they may repeat.
   */
  public static String check(List<String> request) throws IOException {
    List<String> sources = new ArrayList<>();
    List<String> srcjars = new ArrayList<>();
    List<String> classpath = new ArrayList<>();
    List<String> compiler = new ArrayList<>();
    List<String> plugins = new ArrayList<>();
    List<String> pluginOptions = new ArrayList<>();
    List<String> friends = new ArrayList<>();
    List<String> options = new ArrayList<>();
    String module = null;
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
        case "pluginopt":
          pluginOptions.add(value);
          break;
        case "friend":
          friends.add(value);
          break;
        case "kotlincopt":
          options.add(value);
          break;
        case "module":
          module = value;
          break;
        default:
          throw new IOException("unknown request key: " + line.substring(0, split));
      }
    }
    if (compiler.isEmpty()) {
      throw new IOException("request has no compiler= jars");
    }
    Path work = Files.createTempDirectory("kotlinc_check");
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
      args.addAll(List.of("-no-stdlib", "-no-reflect", "-d", classes.toString()));
      args.add("-Xrender-internal-diagnostic-names");
      if (module != null) {
        args.addAll(List.of("-module-name", module));
      }
      if (!classpath.isEmpty()) {
        args.addAll(List.of("-classpath", String.join(File.pathSeparator, classpath)));
      }
      if (!friends.isEmpty()) {
        args.add("-Xfriend-paths=" + String.join(",", friends));
      }
      for (String plugin : plugins) {
        args.add("-Xplugin=" + plugin);
      }
      for (String option : pluginOptions) {
        args.addAll(List.of("-P", option));
      }
      args.addAll(inputs);
      List<Entry> entries = new ArrayList<>();
      int exit = run(compiler, args, entries, names);
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

  private static int run(
      List<String> compilerJars, List<String> args, List<Entry> entries, Map<String, String> names)
      throws IOException {
    URL[] urls = new URL[compilerJars.size()];
    for (int i = 0; i < urls.length; i++) {
      urls[i] = new File(compilerJars.get(i)).toURI().toURL();
    }
    Thread thread = Thread.currentThread();
    ClassLoader previous = thread.getContextClassLoader();
    try (URLClassLoader loader = new URLClassLoader(urls, ClassLoader.getPlatformClassLoader())) {
      thread.setContextClassLoader(loader);
      System.setProperty("idea.io.use.nio2", "true");
      System.setProperty("java.awt.headless", "true");
      Class<?> compilerClass = loader.loadClass("org.jetbrains.kotlin.cli.jvm.K2JVMCompiler");
      Class<?> argumentsClass =
          loader.loadClass("org.jetbrains.kotlin.cli.common.arguments.K2JVMCompilerArguments");
      Class<?> cliCompiler = loader.loadClass("org.jetbrains.kotlin.cli.common.CLICompiler");
      Class<?> collectorClass =
          loader.loadClass("org.jetbrains.kotlin.cli.common.messages.MessageCollector");
      Class<?> servicesClass = loader.loadClass("org.jetbrains.kotlin.config.Services");
      Class<?> commonArguments =
          loader.loadClass("org.jetbrains.kotlin.cli.common.arguments.CommonCompilerArguments");
      Object compiler = compilerClass.getDeclaredConstructor().newInstance();
      Object arguments = argumentsClass.getDeclaredConstructor().newInstance();
      cliCompiler
          .getMethod("parseArguments", String[].class, commonArguments)
          .invoke(compiler, args.toArray(new String[0]), arguments);
      Object collector =
          Proxy.newProxyInstance(
              loader, new Class<?>[] {collectorClass}, new Collector(entries, names));
      Object services = servicesClass.getField("EMPTY").get(null);
      Object result =
          cliCompiler
              .getMethod("exec", collectorClass, servicesClass, commonArguments)
              .invoke(compiler, collector, services, arguments);
      String code = String.valueOf(result);
      if (code.equals("OK")) {
        return 0;
      }
      if (code.equals("COMPILATION_ERROR")) {
        return 1;
      }
      throw new IOException("kotlinc ended with " + code + ": " + describe(entries));
    } catch (InvocationTargetException err) {
      throw new IOException("kotlinc failed: " + err.getCause(), err.getCause());
    } catch (ReflectiveOperationException err) {
      throw new IOException("the compiler jars do not look like kotlinc: " + err, err);
    } finally {
      thread.setContextClassLoader(previous);
    }
  }

  private static String describe(List<Entry> entries) {
    StringBuilder out = new StringBuilder();
    for (Entry e : entries) {
      out.append(e.severity).append(' ').append(e.message).append('\n');
    }
    return out.toString();
  }

  /** The reflective {@code MessageCollector} that records every diagnostic. */
  private static final class Collector implements InvocationHandler {
    private final List<Entry> entries;
    private final Map<String, String> names;
    private boolean errors;

    Collector(List<Entry> entries, Map<String, String> names) {
      this.entries = entries;
      this.names = names;
    }

    @Override
    public Object invoke(Object proxy, Method method, Object[] args) throws Throwable {
      switch (method.getName()) {
        case "report":
          record(args);
          return null;
        case "hasErrors":
          return errors;
        case "clear":
          return null;
        case "hashCode":
          return System.identityHashCode(proxy);
        case "equals":
          return proxy == args[0];
        case "toString":
          return "KotlincCheck.Collector";
        default:
          throw new UnsupportedOperationException(method.toString());
      }
    }

    private void record(Object[] args) throws ReflectiveOperationException {
      String kind = String.valueOf(args[0]);
      String severity;
      switch (kind) {
        case "ERROR":
        case "EXCEPTION":
          severity = "error";
          break;
        case "WARNING":
        case "STRONG_WARNING":
          severity = "warning";
          break;
        case "INFO":
          severity = "note";
          break;
        default:
          return; // LOGGING and OUTPUT are not diagnostics.
      }
      if (severity.equals("error")) {
        errors = true;
      }
      String message = String.valueOf(args[1]);
      String file = "";
      long line = 0;
      long column = 0;
      Object location = args[2];
      if (location != null) {
        Class<?> type = location.getClass();
        String path = (String) type.getMethod("getPath").invoke(location);
        long l = ((Number) type.getMethod("getLine").invoke(location)).longValue();
        long c = ((Number) type.getMethod("getColumn").invoke(location)).longValue();
        if (path != null && l > 0 && c > 0) {
          file = names.getOrDefault(path, relative(path));
          line = l;
          column = c;
        }
      }
      String code = GENERIC_CODE;
      Matcher named = NAMED.matcher(message);
      if (named.matches()) {
        code = named.group(1);
        message = named.group(2);
      } else if (severity.equals("error") && file.isEmpty() && message.contains("-Werror")) {
        code = WERROR_CODE;
      }
      entries.add(new Entry(file, line, column, severity, code, stable(message, names)));
    }
  }

  /** kotlinc reports absolute paths; the runner matches the working-directory-relative ones. */
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
    out.append(",\"tool\":\"kotlinc\",\"exit\":").append(exit);
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
        if (entry.isDirectory()
            || !(name.endsWith(".kt") || name.endsWith(".kts") || name.endsWith(".java"))) {
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
