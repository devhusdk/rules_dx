package conformance;

import com.google.devtools.build.runfiles.Runfiles;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;

public final class RunfilesMain {
  private RunfilesMain() {}

  public static void main(String[] args) throws IOException {
    Runfiles runfiles = Runfiles.preload().withSourceRepository("");
    String path = runfiles.rlocation(args[0]);
    System.out.print(new String(Files.readAllBytes(Paths.get(path)), StandardCharsets.UTF_8));
  }
}
