package dx.quality.kotlinc;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/**
 * Entry point: {@code Main @params} with an {@code output=PATH} line in the params file.
 *
 * <p>Exits 0 whenever kotlinc ran, whatever kotlinc returned, because the log carries kotlinc's own
 * exit status. A non-zero exit means the driver itself failed.
 */
public final class Main {
  private Main() {}

  public static void main(String[] args) {
    try {
      if (args.length != 1 || !args[0].startsWith("@")) {
        throw new IOException("usage: kotlinc_check @params");
      }
      List<String> lines =
          Files.readAllLines(Path.of(args[0].substring(1)), StandardCharsets.UTF_8);
      String output = null;
      List<String> request = new java.util.ArrayList<>();
      for (String line : lines) {
        if (line.startsWith("output=")) {
          output = line.substring("output=".length());
        } else {
          request.add(line);
        }
      }
      if (output == null) {
        throw new IOException("params file has no output= line");
      }
      Files.writeString(Path.of(output), KotlincCheck.check(request), StandardCharsets.UTF_8);
    } catch (IOException | RuntimeException err) {
      System.err.println("kotlinc_check: " + err);
      System.exit(1);
    }
  }
}
