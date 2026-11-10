package dx.quality.scalac;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.regex.Pattern;

/**
 * Runs dotc, the Scala 3 compiler, through its documented {@code Driver.process} entry point.
 *
 * <p>Nothing is compiled against dotty: diagnostics arrive in a {@link Proxy} of the stable {@code
 * dotty.tools.dotc.interfaces} types, and every call goes through those interface methods.
 */
final class Scala3Frontend {
  private static final int ERROR = 2;
  private static final int WARNING = 1;

  /** The "1 warning found" lines dotc prints at the end of a run. */
  private static final Pattern SUMMARY = Pattern.compile("\\d+ (warnings?|errors?) found");

  private Scala3Frontend() {}

  /** Whether the compiler jars hold dotc. */
  static boolean present(ClassLoader loader) {
    return loader.getResource("dotty/tools/dotc/Driver.class") != null;
  }

  /** Compiles the inputs and adds every diagnostic to {@code entries}. */
  static void compile(
      ClassLoader loader,
      List<String> options,
      List<String> inputs,
      List<ScalacCheck.Entry> entries,
      Map<String, String> names)
      throws ReflectiveOperationException {
    Class<?> reporterType = loader.loadClass("dotty.tools.dotc.interfaces.SimpleReporter");
    Class<?> callbackType = loader.loadClass("dotty.tools.dotc.interfaces.CompilerCallback");
    Class<?> diagnosticType = loader.loadClass("dotty.tools.dotc.interfaces.Diagnostic");
    Class<?> positionType = loader.loadClass("dotty.tools.dotc.interfaces.SourcePosition");
    Class<?> sourceType = loader.loadClass("dotty.tools.dotc.interfaces.SourceFile");
    Method level = diagnosticType.getMethod("level");
    Method message = diagnosticType.getMethod("message");
    Method position = diagnosticType.getMethod("position");
    Method line = positionType.getMethod("line");
    Method point = positionType.getMethod("point");
    Method source = positionType.getMethod("source");
    Method path = sourceType.getMethod("path");
    Method content = sourceType.getMethod("content");

    InvocationHandler handler =
        (proxy, method, args) -> {
          if (method.getName().equals("report")) {
            Object diagnostic = args[0];
            try {
              ScalacCheck.Entry found =
                  entry(
                      diagnostic, level, message, position, line, point, source, path, content,
                      names);
              if (!SUMMARY.matcher(found.message()).matches()) {
                entries.add(found);
              }
            } catch (InvocationTargetException err) {
              throw err.getCause();
            }
          }
          return null;
        };
    Object reporter =
        Proxy.newProxyInstance(loader, new Class<?>[] {reporterType}, handler);
    List<String> all = new ArrayList<>(options);
    all.addAll(inputs);
    Class<?> driver = loader.loadClass("dotty.tools.dotc.Driver");
    driver
        .getMethod("process", String[].class, reporterType, callbackType)
        .invoke(driver.getConstructor().newInstance(), all.toArray(new String[0]), reporter, null);
  }

  private static ScalacCheck.Entry entry(
      Object diagnostic,
      Method level,
      Method message,
      Method position,
      Method line,
      Method point,
      Method source,
      Method path,
      Method content,
      Map<String, String> names)
      throws ReflectiveOperationException {
    int levelValue = ((Number) level.invoke(diagnostic)).intValue();
    String severity = levelValue == ERROR ? "error" : levelValue == WARNING ? "warning" : "note";
    String text = String.valueOf(message.invoke(diagnostic));
    String file = "";
    long lineNumber = 0;
    long column = 0;
    Optional<?> pos = (Optional<?>) position.invoke(diagnostic);
    if (pos.isPresent()) {
      Object at = pos.get();
      Object src = source.invoke(at);
      String name = String.valueOf(path.invoke(src));
      char[] chars = (char[]) content.invoke(src);
      int offset = ((Number) point.invoke(at)).intValue();
      if (offset >= 0 && offset <= chars.length) {
        int start = offset;
        while (start > 0 && chars[start - 1] != '\n') {
          start--;
        }
        file = names.getOrDefault(name, ScalacCheck.relative(name));
        lineNumber = ((Number) line.invoke(at)).longValue() + 1;
        column = offset - start + 1;
      }
    }
    String code = ScalacCheck.GENERIC_CODE;
    if (severity.equals("error") && file.isEmpty() && text.contains("-Werror")) {
      code = ScalacCheck.WERROR_CODE;
    }
    return new ScalacCheck.Entry(
        file, lineNumber, column, severity, code, ScalacCheck.stable(text, names));
  }
}
