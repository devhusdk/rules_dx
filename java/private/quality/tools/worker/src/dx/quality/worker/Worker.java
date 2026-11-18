package dx.quality.worker;

import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Shared entry point of the JVM check drivers, as a one-shot command or a Bazel persistent worker.
 *
 * <p>One-shot: {@code tool @params}. Worker: Bazel starts {@code tool --persistent_worker} and sends
 * JSON {@code WorkRequest}s on stdin whose {@code arguments} are the params file lines (Bazel expands
 * the trailing {@code @params} into the request). The actions declare a singleplex worker, so
 * requests arrive one at a time and, under {@code --worker_sandboxing}, the working directory is the
 * worker's sandbox.
 *
 * <p>Each request is a list of {@code key=value} lines with exactly one {@code output=PATH}; the
 * check's rendered log is written there. A check's own failure is a non-zero exit (one-shot) or a
 * non-zero {@code exitCode} (worker), never a crashed worker.
 */
public final class Worker {
  private Worker() {}

  /** One check run: the request lines without {@code output=}, returning the rendered log. */
  @FunctionalInterface
  public interface Check {
    String run(List<String> request) throws IOException;
  }

  public static void main(String name, String[] args, Check check) {
    if (args.length == 1 && args[0].equals("--persistent_worker")) {
      // stdout carries the protocol; anything a compiler prints there would corrupt it.
      PrintStream protocol = System.out;
      try {
        serve(name, System.in, protocol, check);
      } catch (IOException | RuntimeException err) {
        System.err.println(name + ": worker stopped: " + err);
        System.exit(1);
      }
      return;
    }
    try {
      if (args.length != 1 || !args[0].startsWith("@")) {
        throw new IOException("usage: " + name + " @params | --persistent_worker");
      }
      handle(Files.readAllLines(Path.of(args[0].substring(1)), StandardCharsets.UTF_8), check);
    } catch (IOException | RuntimeException err) {
      System.err.println(name + ": " + err);
      System.exit(1);
    }
  }

  /** Answers work requests from {@code in} on {@code out} until {@code in} ends. */
  public static void serve(String name, InputStream in, OutputStream out, Check check)
      throws IOException {
    Reader reader = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8));
    PrintStream stdout = System.out;
    PrintStream stderr = System.err;
    while (true) {
      Object message = Json.read(reader);
      if (message == null) {
        return;
      }
      if (!(message instanceof Map<?, ?> request)) {
        throw new IOException("work request is not a JSON object");
      }
      List<String> lines = new ArrayList<>();
      if (request.get("arguments") instanceof List<?> arguments) {
        for (Object argument : arguments) {
          lines.add(String.valueOf(argument));
        }
      }
      long id = request.get("requestId") instanceof Number n ? n.longValue() : 0;
      ByteArrayOutputStream captured = new ByteArrayOutputStream();
      PrintStream capture = new PrintStream(captured, true, StandardCharsets.UTF_8);
      int exit = 0;
      System.setOut(capture);
      System.setErr(capture);
      try {
        handle(lines, check);
      } catch (IOException | RuntimeException err) {
        capture.println(name + ": " + err);
        exit = 1;
      } finally {
        System.setOut(stdout);
        System.setErr(stderr);
      }
      capture.flush();
      String response =
          "{\"exitCode\":"
              + exit
              + ",\"output\":"
              + Json.quote(captured.toString(StandardCharsets.UTF_8))
              + ",\"requestId\":"
              + id
              + "}\n";
      out.write(response.getBytes(StandardCharsets.UTF_8));
      out.flush();
    }
  }

  private static void handle(List<String> lines, Check check) throws IOException {
    String output = null;
    List<String> request = new ArrayList<>();
    for (String line : lines) {
      if (line.startsWith("output=")) {
        output = line.substring("output=".length());
      } else {
        request.add(line);
      }
    }
    if (output == null) {
      throw new IOException("request has no output= line");
    }
    Files.writeString(Path.of(output), check.run(request), StandardCharsets.UTF_8);
  }
}
