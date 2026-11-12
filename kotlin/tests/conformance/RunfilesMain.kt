package conformance

import com.google.devtools.build.runfiles.Runfiles
import java.io.File

fun main(args: Array<String>) {
    val runfiles = Runfiles.preload().withSourceRepository("")
    print(File(runfiles.rlocation(args[0])).readText())
}
