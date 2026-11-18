package dx.quality.worker;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Self-checking test for {@link Worker}; exits non-zero on the first failed expectation. */
public final class WorkerTest {
  private WorkerTest() {}

  public static void main(String[] args) throws IOException {
    Path dir = Files.createTempDirectory("worker_test");
    Path first = dir.resolve("first.json");
    Path second = dir.resolve("second.json");
    // Bazel may pretty-print a request over several lines; the second is on one line.
    String requests =
        "{\n  \"arguments\": [\"output="
            + json(first)
            + "\", \"source=a \\\"b\\\"\\u00e9\"],\n  \"requestId\": 0\n}\n"
            + "{\"arguments\":[\"output="
            + json(second)
            + "\",\"fail\"],\"inputs\":[{\"path\":\"x\",\"digest\":\"YQ==\"}],\"requestId\":0}\n";
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    Worker.serve(
        "worker_test",
        new ByteArrayInputStream(requests.getBytes(StandardCharsets.UTF_8)),
        out,
        WorkerTest::check);
    String[] responses = out.toString(StandardCharsets.UTF_8).split("\n");
    expect(responses.length == 2, "two responses", out.toString(StandardCharsets.UTF_8));

    Map<?, ?> ok = (Map<?, ?>) Json.read(new java.io.StringReader(responses[0]));
    expect(Long.valueOf(0).equals(ok.get("exitCode")), "first request succeeds", responses[0]);
    expect(("printed" + System.lineSeparator()).equals(ok.get("output")), "stdout goes to the response", responses[0]);
    expect(
        "source=a \"b\"\u00e9".equals(Files.readString(first)),
        "log written to output=",
        Files.readString(first));

    Map<?, ?> failed = (Map<?, ?>) Json.read(new java.io.StringReader(responses[1]));
    expect(Long.valueOf(1).equals(failed.get("exitCode")), "check failure is exit 1", responses[1]);
    expect(
        String.valueOf(failed.get("output")).contains("worker_test: java.io.IOException: boom"),
        "failure is reported",
        responses[1]);
    expect(!Files.exists(second), "no log on failure", second.toString());
    System.out.println("ok");
  }

  private static String check(List<String> request) throws IOException {
    if (request.contains("fail")) {
      throw new IOException("boom");
    }
    System.out.println("printed");
    return String.join("\n", request);
  }

  private static String json(Path path) {
    return path.toString().replace("\\", "\\\\");
  }

  private static void expect(boolean condition, String what, String detail) {
    if (!condition) {
      throw new AssertionError(what + ": " + detail);
    }
  }
}
