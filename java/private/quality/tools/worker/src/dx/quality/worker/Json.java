package dx.quality.worker;

import java.io.IOException;
import java.io.Reader;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * The little JSON the worker protocol needs: read one value from a stream, quote one string.
 *
 * <p>Bazel may write a request across several lines, so values are read by structure, not by line.
 * Numbers come back as {@link Double} or {@link Long}, objects as {@link Map}, arrays as {@link
 * List}.
 */
final class Json {
  private final Reader in;
  private int peeked = -2;

  private Json(Reader in) {
    this.in = in;
  }

  /** The next value, or null when the stream ends before one starts. */
  static Object read(Reader in) throws IOException {
    Json json = new Json(in);
    if (json.skipSpace() == -1) {
      return null;
    }
    return json.value();
  }

  static String quote(String text) {
    StringBuilder out = new StringBuilder(text.length() + 2).append('"');
    for (int i = 0; i < text.length(); i++) {
      char c = text.charAt(i);
      switch (c) {
        case '"' -> out.append("\\\"");
        case '\\' -> out.append("\\\\");
        case '\n' -> out.append("\\n");
        case '\r' -> out.append("\\r");
        case '\t' -> out.append("\\t");
        default -> {
          if (c < 0x20) {
            out.append(String.format("\\u%04x", (int) c));
          } else {
            out.append(c);
          }
        }
      }
    }
    return out.append('"').toString();
  }

  private int peek() throws IOException {
    if (peeked == -2) {
      peeked = in.read();
    }
    return peeked;
  }

  private int next() throws IOException {
    int c = peek();
    peeked = -2;
    return c;
  }

  private int skipSpace() throws IOException {
    while (peek() == ' ' || peek() == '\n' || peek() == '\r' || peek() == '\t') {
      next();
    }
    return peek();
  }

  private void expect(char want) throws IOException {
    int got = next();
    if (got != want) {
      throw new IOException("bad JSON: expected '" + want + "', got " + describe(got));
    }
  }

  private static String describe(int c) {
    return c == -1 ? "end of input" : "'" + (char) c + "'";
  }

  private Object value() throws IOException {
    int c = skipSpace();
    switch (c) {
      case '{':
        return object();
      case '[':
        return array();
      case '"':
        return string();
      case 't':
        word("true");
        return Boolean.TRUE;
      case 'f':
        word("false");
        return Boolean.FALSE;
      case 'n':
        word("null");
        return null;
      default:
        if (c == '-' || (c >= '0' && c <= '9')) {
          return number();
        }
        throw new IOException("bad JSON: unexpected " + describe(c));
    }
  }

  private Map<String, Object> object() throws IOException {
    expect('{');
    Map<String, Object> map = new LinkedHashMap<>();
    if (skipSpace() == '}') {
      next();
      return map;
    }
    while (true) {
      skipSpace();
      String key = string();
      skipSpace();
      expect(':');
      map.put(key, value());
      int c = skipSpace();
      next();
      if (c == '}') {
        return map;
      }
      if (c != ',') {
        throw new IOException("bad JSON: expected ',' or '}', got " + describe(c));
      }
    }
  }

  private List<Object> array() throws IOException {
    expect('[');
    List<Object> list = new ArrayList<>();
    if (skipSpace() == ']') {
      next();
      return list;
    }
    while (true) {
      list.add(value());
      int c = skipSpace();
      next();
      if (c == ']') {
        return list;
      }
      if (c != ',') {
        throw new IOException("bad JSON: expected ',' or ']', got " + describe(c));
      }
    }
  }

  private String string() throws IOException {
    expect('"');
    StringBuilder out = new StringBuilder();
    while (true) {
      int c = next();
      switch (c) {
        case -1 -> throw new IOException("bad JSON: unterminated string");
        case '"' -> {
          return out.toString();
        }
        case '\\' -> {
          int e = next();
          switch (e) {
            case '"', '\\', '/' -> out.append((char) e);
            case 'b' -> out.append('\b');
            case 'f' -> out.append('\f');
            case 'n' -> out.append('\n');
            case 'r' -> out.append('\r');
            case 't' -> out.append('\t');
            case 'u' -> {
              char[] hex = new char[4];
              for (int i = 0; i < 4; i++) {
                int h = next();
                if (h == -1) {
                  throw new IOException("bad JSON: unterminated escape");
                }
                hex[i] = (char) h;
              }
              try {
                out.append((char) Integer.parseInt(new String(hex), 16));
              } catch (NumberFormatException err) {
                throw new IOException("bad JSON: bad escape \\u" + new String(hex), err);
              }
            }
            default -> throw new IOException("bad JSON: bad escape " + describe(e));
          }
        }
        default -> out.append((char) c);
      }
    }
  }

  private Object number() throws IOException {
    StringBuilder text = new StringBuilder();
    while (true) {
      int c = peek();
      if (c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' || (c >= '0' && c <= '9')) {
        text.append((char) next());
      } else {
        break;
      }
    }
    String s = text.toString();
    try {
      if (s.contains(".") || s.contains("e") || s.contains("E")) {
        return Double.parseDouble(s);
      }
      return Long.parseLong(s);
    } catch (NumberFormatException err) {
      throw new IOException("bad JSON: bad number " + s, err);
    }
  }

  private void word(String word) throws IOException {
    for (int i = 0; i < word.length(); i++) {
      int c = next();
      if (c != word.charAt(i)) {
        throw new IOException("bad JSON: expected " + word + ", got " + describe(c));
      }
    }
  }
}
