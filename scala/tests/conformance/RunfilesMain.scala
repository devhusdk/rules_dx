package conformance

import com.google.devtools.build.runfiles.Runfiles
import java.nio.file.{Files, Paths}

object RunfilesMain {
  def main(args: Array[String]): Unit = {
    val runfiles = Runfiles.preload().withSourceRepository("")
    print(new String(Files.readAllBytes(Paths.get(runfiles.rlocation(args(0)))), "UTF-8"))
  }
}
