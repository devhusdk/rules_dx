package dx.bench;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Collection;
import java.util.Locale;
import org.openjdk.jmh.results.RunResult;
import org.openjdk.jmh.runner.Runner;
import org.openjdk.jmh.runner.RunnerException;
import org.openjdk.jmh.runner.options.Options;
import org.openjdk.jmh.runner.options.OptionsBuilder;
import org.openjdk.jmh.runner.options.TimeValue;

/** Runs every JMH benchmark on the classpath and writes dx-bench.json. */
public final class JmhRunner {
  private JmhRunner() {}

  public static void main(String[] args) throws IOException, RunnerException {
    Options options =
        new OptionsBuilder()
            .forks(0)
            .warmupIterations(1)
            .warmupTime(TimeValue.milliseconds(200))
            .measurementIterations(3)
            .measurementTime(TimeValue.milliseconds(200))
            .build();
    Collection<RunResult> results = new Runner(options).run();
    String outputs = System.getenv("TEST_UNDECLARED_OUTPUTS_DIR");
    Path dir = Paths.get(outputs == null ? "." : outputs);
    Files.createDirectories(dir);
    Files.writeString(dir.resolve("dx-bench.json"), toJson(results), StandardCharsets.UTF_8);
  }

  static String toJson(Collection<RunResult> results) {
    StringBuilder json = new StringBuilder("{\"schema\":\"dx-bench/v1\",\"results\":[");
    boolean first = true;
    for (RunResult result : results) {
      if (!first) {
        json.append(',');
      }
      first = false;
      json.append(
          row(
              result.getParams().getBenchmark(),
              result.getPrimaryResult().getScoreUnit(),
              result.getPrimaryResult().getScore(),
              result.getPrimaryResult().getStatistics().getStandardDeviation(),
              result.getPrimaryResult().getStatistics().getN()));
    }
    return json.append("]}").toString();
  }

  static String row(String name, String unit, double mean, double stddev, long iterations) {
    return String.format(
        Locale.ROOT,
        "{\"name\":\"%s\",\"unit\":\"%s\",\"mean\":%s,\"stddev\":%s,\"iterations\":%d}",
        escape(name),
        escape(unit),
        number(mean),
        number(stddev),
        iterations);
  }

  private static String number(double value) {
    return Double.isFinite(value) ? Double.toString(value) : "0.0";
  }

  private static String escape(String text) {
    return text.replace("\\", "\\\\").replace("\"", "\\\"");
  }
}
