package dx.docs.jvm;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ApiRecordWriterTest {
  private static ApiRecordWriter.Symbol type(String qualified, String doc) {
    return new ApiRecordWriter.Symbol(
        qualified, null, "class", "class X", doc, List.of(), null, "a/X.java", 3, null, List.of());
  }

  @Test
  void escapesQuotesBackslashesControlsAndKeepsAstralCharacters() {
    String json =
        ApiRecordWriter.render(
            "java", "javadoc", "21", List.of(type("a.X", "q\" b\\ n\n t\t c\u0001 😀")));
    assertTrue(json.contains("\"doc\":\"q\\\" b\\\\ n\\n t\\t c\\u0001 😀\""), json);
  }

  @Test
  void escapesLoneSurrogates() {
    String json = ApiRecordWriter.render("java", "javadoc", "21", List.of(type("a.X", "x\uD83Dy")));
    assertTrue(json.contains("x\\ud83dy"), json);
  }

  @Test
  void leavesOutOptionalKeys() {
    String json = ApiRecordWriter.render("java", "javadoc", "21", List.of(type("a.X", "d")));
    assertFalse(json.contains("overload"), json);
    assertFalse(json.contains("returns"), json);
    assertFalse(json.contains("params"), json);
    assertFalse(json.contains("member_of"), json);
    assertFalse(json.contains("implements"), json);
    assertTrue(json.startsWith("{\"format\":1,\"language\":\"java\""), json);
  }

  @Test
  void writesMethodFields() {
    ApiRecordWriter.Symbol method =
        new ApiRecordWriter.Symbol(
            "a.X.m",
            List.of("int", "String[]"),
            "method",
            "int m(int a, String... b)",
            "doc",
            List.of(new ApiRecordWriter.Param("a", "int", "the a")),
            new ApiRecordWriter.Returns("int", "the result"),
            "a/X.java",
            9,
            "a.X",
            List.of("a.Y"));
    String json = ApiRecordWriter.render("java", "javadoc", "21", List.of(method));
    assertTrue(json.contains("\"overload\":[\"int\",\"String[]\"]"), json);
    assertTrue(json.contains("\"params\":[{\"name\":\"a\",\"type\":\"int\",\"doc\":\"the a\"}]"));
    assertTrue(json.contains("\"returns\":{\"type\":\"int\",\"doc\":\"the result\"}"));
    assertTrue(json.contains("\"line\":9,\"member_of\":\"a.X\",\"implements\":[\"a.Y\"]"), json);
  }

  @Test
  void writesTheReceiverOfAnExtensionPropertyAndOrdersByIt() {
    ApiRecordWriter.Symbol circle = receiverProperty("Circle");
    ApiRecordWriter.Symbol square = receiverProperty("Square");
    String json = ApiRecordWriter.render("kotlin", "dokka", "2.2.0", List.of(square, circle));
    assertTrue(json.contains("\"receiver\":\"Circle\",\"kind\":\"property\""), json);
    assertTrue(json.indexOf("\"Circle\"") < json.indexOf("\"Square\""), json);
  }

  private static ApiRecordWriter.Symbol receiverProperty(String receiver) {
    return new ApiRecordWriter.Symbol(
        "a.area",
        null,
        "property",
        "val area: Double",
        "",
        null,
        null,
        "a/X.kt",
        4,
        "a",
        List.of(),
        receiver);
  }

  @Test
  void ordersSymbolsStablyWhateverTheInputOrder() {
    ApiRecordWriter.Symbol a = type("a.A", "");
    ApiRecordWriter.Symbol b = type("a.B", "");
    assertEquals(
        ApiRecordWriter.render("java", "javadoc", "21", List.of(a, b)),
        ApiRecordWriter.render("java", "javadoc", "21", List.of(b, a)));
  }

  @Test
  void writesAtomically(@TempDir Path dir) throws IOException {
    Path out = dir.resolve("api.json");
    ApiRecordWriter.write(out, "java", "javadoc", "21", List.of(type("a.X", "d")));
    assertTrue(Files.readString(out, StandardCharsets.UTF_8).contains("\"qualified\":\"a.X\""));
    try (var names = Files.list(dir)) {
      assertEquals(1, names.count());
    }
  }
}
