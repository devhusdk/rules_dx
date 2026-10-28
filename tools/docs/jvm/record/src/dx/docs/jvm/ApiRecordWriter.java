package dx.docs.jvm;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/**
 * Writes the "jvm api v1" JSON that {@code jvm_convert} reads, for every JVM doc adapter.
 *
 * <p>Contract shared by the Java, Kotlin and Scala adapters:
 *
 * <ul>
 *   <li>The document has {@code format} 1, {@code language}, {@code tool}, {@code tool_version} and
 *       {@code symbols}.
 *   <li>A symbol's {@code qualified} name joins packages and types with {@code .}; nested types use
 *       {@code .} too; a constructor is {@code pkg.Type.Type}.
 *   <li>Constructors and methods carry an {@code overload} list: the erased simple type names of
 *       the parameters, with arrays and varargs written {@code T[]}. The reader joins them with
 *       {@code ,} and no spaces to form the key {@code pkg.Type.name(int,String)}.
 *   <li>Links inside docs are {@code [label](jvm:<qualified>[(T,...)])}, using that same key.
 *   <li>A Kotlin extension property also carries {@code receiver}, the erased simple receiver type.
 *   <li>{@code file} is the exact execution path of the source from the srcs manifest.
 * </ul>
 */
public final class ApiRecordWriter {
  /** One documented parameter. */
  public static final class Param {
    public final String name;
    public final String type;
    public final String doc;

    public Param(String name, String type, String doc) {
      this.name = name;
      this.type = type;
      this.doc = doc;
    }
  }

  /** The documented return value. */
  public static final class Returns {
    public final String type;
    public final String doc;

    public Returns(String type, String doc) {
      this.type = type;
      this.doc = doc;
    }
  }

  /**
   * One public symbol. {@code overload}, {@code returns} and {@code memberOf} are null when absent.
   */
  public static final class Symbol {
    public final String qualified;
    public final List<String> overload;
    public final String kind;
    public final String signature;
    public final String doc;
    public final List<Param> params;
    public final Returns returns;
    public final String file;
    public final int line;
    public final String memberOf;
    public final List<String> implementsTypes;
    public final String receiver;

    public Symbol(
        String qualified,
        List<String> overload,
        String kind,
        String signature,
        String doc,
        List<Param> params,
        Returns returns,
        String file,
        int line,
        String memberOf,
        List<String> implementsTypes) {
      this(
          qualified,
          overload,
          kind,
          signature,
          doc,
          params,
          returns,
          file,
          line,
          memberOf,
          implementsTypes,
          null);
    }

    /** As above, with the erased receiver type of a Kotlin extension property, or null. */
    public Symbol(
        String qualified,
        List<String> overload,
        String kind,
        String signature,
        String doc,
        List<Param> params,
        Returns returns,
        String file,
        int line,
        String memberOf,
        List<String> implementsTypes,
        String receiver) {
      this.qualified = qualified;
      this.overload = overload;
      this.kind = kind;
      this.signature = signature;
      this.doc = doc;
      this.params = params;
      this.returns = returns;
      this.file = file;
      this.line = line;
      this.memberOf = memberOf;
      this.implementsTypes = implementsTypes;
      this.receiver = receiver;
    }
  }

  private ApiRecordWriter() {}

  /** Renders one document with its symbols in a stable order. */
  public static String render(
      String language, String tool, String toolVersion, List<Symbol> symbols) {
    List<Symbol> sorted = new ArrayList<>(symbols);
    sorted.sort(
        Comparator.comparing((Symbol symbol) -> symbol.qualified)
            .thenComparing((Symbol symbol) -> key(symbol.overload))
            .thenComparing((Symbol symbol) -> symbol.kind)
            .thenComparing((Symbol symbol) -> symbol.receiver == null ? "" : symbol.receiver));
    StringBuilder out = new StringBuilder();
    out.append("{\"format\":1,\"language\":");
    quote(out, language);
    out.append(",\"tool\":");
    quote(out, tool);
    out.append(",\"tool_version\":");
    quote(out, toolVersion);
    out.append(",\"symbols\":[");
    for (int index = 0; index < sorted.size(); index++) {
      if (index > 0) {
        out.append(',');
      }
      symbol(out, sorted.get(index));
    }
    out.append("]}\n");
    return out.toString();
  }

  /** Writes the document to a temporary sibling and renames it into place on success. */
  public static void write(
      Path out, String language, String tool, String toolVersion, List<Symbol> symbols)
      throws IOException {
    Path temp = out.resolveSibling(out.getFileName() + ".tmp");
    try {
      Files.writeString(temp, render(language, tool, toolVersion, symbols), StandardCharsets.UTF_8);
      Files.move(temp, out, StandardCopyOption.REPLACE_EXISTING);
    } finally {
      Files.deleteIfExists(temp);
    }
  }

  private static String key(List<String> overload) {
    return overload == null ? "" : "(" + String.join(",", overload) + ")";
  }

  private static void symbol(StringBuilder out, Symbol symbol) {
    out.append("{\"qualified\":");
    quote(out, symbol.qualified);
    if (symbol.overload != null) {
      out.append(",\"overload\":");
      strings(out, symbol.overload);
    }
    if (symbol.receiver != null) {
      out.append(",\"receiver\":");
      quote(out, symbol.receiver);
    }
    out.append(",\"kind\":");
    quote(out, symbol.kind);
    out.append(",\"signature\":");
    quote(out, symbol.signature);
    out.append(",\"doc\":");
    quote(out, symbol.doc);
    if (symbol.params != null && !symbol.params.isEmpty()) {
      out.append(",\"params\":[");
      boolean first = true;
      for (Param param : symbol.params) {
        if (!first) {
          out.append(',');
        }
        first = false;
        out.append("{\"name\":");
        quote(out, param.name);
        out.append(",\"type\":");
        quote(out, param.type);
        out.append(",\"doc\":");
        quote(out, param.doc);
        out.append('}');
      }
      out.append(']');
    }
    if (symbol.returns != null) {
      out.append(",\"returns\":{\"type\":");
      quote(out, symbol.returns.type);
      out.append(",\"doc\":");
      quote(out, symbol.returns.doc);
      out.append('}');
    }
    out.append(",\"file\":");
    quote(out, symbol.file);
    out.append(",\"line\":").append(symbol.line);
    if (symbol.memberOf != null) {
      out.append(",\"member_of\":");
      quote(out, symbol.memberOf);
    }
    if (symbol.implementsTypes != null && !symbol.implementsTypes.isEmpty()) {
      out.append(",\"implements\":");
      strings(out, symbol.implementsTypes);
    }
    out.append('}');
  }

  private static void strings(StringBuilder out, List<String> values) {
    out.append('[');
    for (int index = 0; index < values.size(); index++) {
      if (index > 0) {
        out.append(',');
      }
      quote(out, values.get(index));
    }
    out.append(']');
  }

  private static void quote(StringBuilder out, String text) {
    out.append('"');
    for (int index = 0; index < text.length(); index++) {
      char ch = text.charAt(index);
      if (ch == '"') {
        out.append("\\\"");
      } else if (ch == '\\') {
        out.append("\\\\");
      } else if (ch == '\n') {
        out.append("\\n");
      } else if (ch == '\r') {
        out.append("\\r");
      } else if (ch == '\t') {
        out.append("\\t");
      } else if (ch == '\b') {
        out.append("\\b");
      } else if (ch == '\f') {
        out.append("\\f");
      } else if (ch < 0x20 || ch == 0x7f || lone(text, index)) {
        out.append(String.format("\\u%04x", (int) ch));
      } else {
        out.append(ch);
      }
    }
    out.append('"');
  }

  private static boolean lone(String text, int index) {
    char ch = text.charAt(index);
    if (Character.isHighSurrogate(ch)) {
      return index + 1 >= text.length() || !Character.isLowSurrogate(text.charAt(index + 1));
    }
    if (Character.isLowSurrogate(ch)) {
      return index == 0 || !Character.isHighSurrogate(text.charAt(index - 1));
    }
    return false;
  }
}
